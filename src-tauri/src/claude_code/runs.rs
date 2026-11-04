//! The runs themselves: starting one, following it while it works, stopping it, and putting the vault in order when
//! it is over. Each run has a thread of its own, so a run goes on whatever the person does in the app meanwhile.
//! Runs are kept in memory (the latest few) for the screen, and when one is over a note about it is kept in its thread
//! (see `records`).
//!
//! Everything that talks to the outside world is behind a trait: [`Runner`] starts the process, [`Host`] tells the
//! window what is happening and brings the vault up to date. The tests give both fakes.

use std::ffi::OsString;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::acp;
use super::cli::{self, Search};
use super::config::{self, ClaudeSettings, Provider};
use super::prompt::RunRequest;
use super::records::{self, RunFacts, RunRecord};
use super::redact::Redactor;
use super::render::{self, ResultInfo, Step};
use super::runner::{self, Exit, Process, Runner, SpawnSpec};
use super::versions::{Baseline, Change};
use crate::error::{Error, Result};
use crate::{deliverables, ids, settings};

/// How many runs may be going at once.
pub const MAX_CONCURRENT: usize = 3;
/// How many finished runs the screen can still show.
const KEPT_RUNS: usize = 30;
/// How much output is kept in memory for the screen.
pub const MAX_OUTPUT_BYTES: usize = 512 * 1024;
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
/// How many steps (tool calls) are remembered for the note about the run; it lists fewer than this.
const MAX_STEPS_KEPT: usize = 2000;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- what the screen sees ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Finished,
    Failed,
    Cancelled,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Finished => "finished",
            Status::Failed => "failed",
            Status::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub provider: String,
    pub id: String,
    pub project: String,
    pub thread: String,
    pub prompt_id: String,
    pub prompt_title: String,
    pub status: Status,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
    /// Why it failed or stopped, in a sentence.
    pub message: Option<String>,
    /// Cancel was pressed and the process is being stopped.
    pub cancelling: bool,
    /// How many pieces of output there have been: the screen compares it with the pieces it has seen.
    pub chunks: u64,
    /// Deliverables the app linked to the prompt because the run created them.
    pub linked: Vec<String>,
    /// Files changed on disk and the vault was reloaded.
    pub vault_reloaded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSnapshot {
    pub run: RunView,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub seq: u64,
    pub text: String,
}

/// The `claude-run` event: the run as it now is, and the piece of output that was just added, if any.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub run: RunView,
    pub chunk: Option<Chunk>,
}

/// What putting the vault in order after a run found.
#[derive(Debug, Clone, Default)]
pub struct VaultOutcome {
    pub reloaded: bool,
    /// The versions the run's deliverables were published as and linked to the prompt, like "plan@v0.2.md".
    pub linked: Vec<String>,
    /// Versions made to keep edits that no version held, before the run changed the file. They are not linked.
    pub kept: Vec<String>,
    pub error: Option<String>,
}

/// The world outside the runs: the window, and the vault.
pub trait Host: Send + Sync {
    fn emit(&self, event: &RunEvent);
    /// The process is gone: read in what it wrote, keep any earlier edits it changed as a version, and (when `link`)
    /// publish what the run made of each deliverable and link that version to the prompt.
    fn vault_after_run(&self, request: &RunRequest, changes: &[Change], link: bool) -> VaultOutcome;
    /// Keeps the note about the run in its thread. The error is a sentence for the run's output.
    fn keep_record(&self, _record: &RunRecord) -> std::result::Result<(), String> {
        Ok(())
    }
}

// ---- one run ----------------------------------------------------------------------------------------

struct State {
    status: Status,
    output: String,
    chunks: u64,
    truncated: bool,
    finished_at: Option<String>,
    exit_code: Option<i32>,
    message: Option<String>,
    linked: Vec<String>,
    vault_reloaded: bool,
    result: Option<ResultInfo>,
    terminal: bool,
    /// What the run did, in order, and the model it started with: the note about the run is made from these.
    steps: Vec<Step>,
    model: Option<String>,
}

struct Run {
    provider: Provider,
    id: String,
    project: String,
    thread: String,
    prompt_id: String,
    prompt_title: String,
    started_at: String,
    started: Instant,
    cancel: AtomicBool,
    /// The program printed "unknown option": the Claude Code installed is probably older than the flags used.
    unknown_option: AtomicBool,
    process: Mutex<Option<Arc<dyn Process>>>,
    state: Mutex<State>,
}

impl Run {
    fn new(request: &RunRequest, provider: Provider) -> Run {
        Run {
            provider,
            id: ids::new_id(),
            project: request.project.clone(),
            thread: request.thread.clone(),
            prompt_id: request.prompt_id.clone(),
            prompt_title: request.prompt_title.clone(),
            started_at: ids::now(),
            started: Instant::now(),
            cancel: AtomicBool::new(false),
            unknown_option: AtomicBool::new(false),
            process: Mutex::new(None),
            state: Mutex::new(State {
                status: Status::Running,
                output: String::new(),
                chunks: 0,
                truncated: false,
                finished_at: None,
                exit_code: None,
                message: None,
                linked: Vec::new(),
                vault_reloaded: false,
                result: None,
                terminal: false,
                steps: Vec::new(),
                model: None,
            }),
        }
    }

    fn view_of(&self, state: &State) -> RunView {
        RunView {
            provider: self.provider.as_str().to_string(),
            id: self.id.clone(),
            project: self.project.clone(),
            thread: self.thread.clone(),
            prompt_id: self.prompt_id.clone(),
            prompt_title: self.prompt_title.clone(),
            status: state.status,
            started_at: self.started_at.clone(),
            finished_at: state.finished_at.clone(),
            exit_code: state.exit_code,
            message: state.message.clone(),
            cancelling: state.status == Status::Running && self.cancel.load(Ordering::SeqCst),
            chunks: state.chunks,
            linked: state.linked.clone(),
            vault_reloaded: state.vault_reloaded,
        }
    }

    fn view(&self) -> RunView {
        self.view_of(&lock(&self.state))
    }

    fn snapshot(&self) -> RunSnapshot {
        let state = lock(&self.state);
        RunSnapshot { run: self.view_of(&state), output: state.output.clone() }
    }

    fn is_running(&self) -> bool {
        lock(&self.state).status == Status::Running
    }

    fn is_for(&self, request: &RunRequest) -> bool {
        self.project == request.project && self.thread == request.thread && self.prompt_id == request.prompt_id
    }
}

/// Where a run's output goes: the screen (through the host), with secrets taken out on the way.
struct Sink {
    run: Arc<Run>,
    host: Arc<dyn Host>,
    redactor: Redactor,
}

impl Sink {
    /// Adds text to the output and the screen. Everything is done under the run's lock, so pieces reach the screen in
    /// the order they were added whichever stream they came from.
    fn push(&self, text: &str) {
        let text = self.redactor.apply(text);
        if text.is_empty() {
            return;
        }
        let mut state = lock(&self.run.state);
        let piece = if state.truncated {
            String::new()
        } else if state.output.len() + text.len() > MAX_OUTPUT_BYTES {
            state.truncated = true;
            "… The output is cut here.\n".to_string()
        } else {
            text
        };
        if piece.is_empty() {
            return;
        }
        state.output.push_str(&piece);
        let seq = state.chunks;
        state.chunks += 1;
        self.host.emit(&RunEvent { run: self.run.view_of(&state), chunk: Some(Chunk { seq, text: piece }) });
    }
}

// ---- the manager ------------------------------------------------------------------------------------

/// What a run needs besides the prompt: where the vault is and the settings it runs under.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub root: PathBuf,
    pub settings: ClaudeSettings,
}

struct Inner {
    runner: Arc<dyn Runner>,
    host: Arc<dyn Host>,
    runs: Mutex<Vec<Arc<Run>>>,
    /// Ledger is about to restart to install an update: no run may start. Read and set under the `runs` lock, so a
    /// run either is in the list when the hold is taken or doesn't start.
    held: AtomicBool,
}

#[derive(Clone)]
pub struct Runs {
    inner: Arc<Inner>,
}

impl Runs {
    pub fn new(runner: Arc<dyn Runner>, host: Arc<dyn Host>) -> Runs {
        Runs { inner: Arc::new(Inner { runner, host, runs: Mutex::new(Vec::new()), held: AtomicBool::new(false) }) }
    }

    /// The runs still going.
    pub fn running(&self) -> Vec<RunView> {
        lock(&self.inner.runs).iter().filter(|r| r.is_running()).map(|r| r.view()).collect()
    }

    /// For a restart: succeeds when no run is going, and from then on none can start until [`Runs::release_hold`].
    /// Otherwise the runs still going.
    pub fn hold_for_restart(&self) -> std::result::Result<(), Vec<RunView>> {
        let runs = lock(&self.inner.runs);
        let going: Vec<RunView> = runs.iter().filter(|r| r.is_running()).map(|r| r.view()).collect();
        if !going.is_empty() {
            return Err(going);
        }
        self.inner.held.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// The restart didn't happen after all: runs can start again.
    pub fn release_hold(&self) {
        let _runs = lock(&self.inner.runs);
        self.inner.held.store(false, Ordering::SeqCst);
    }

    /// Starts a run in the background. Fails, without starting anything, if Claude Code can't be found, if this prompt
    /// is already running, or if the limit on runs at once is reached.
    pub fn start(&self, mut request: RunRequest, ctx: RunContext, search: &Search) -> Result<RunView> {
        // Checked again under the lock below; this saves writing the guard file for nothing.
        if self.inner.held.load(Ordering::SeqCst) {
            return Err(restarting());
        }
        cli::validate_adapter(&ctx.settings)?;
        if ctx.settings.provider == Provider::Prime && request.cwd.to_str().is_none() {
            return Err(Error::invalid("Prime Agent requires a UTF-8 working directory"));
        }
        let program = cli::resolve_for(ctx.settings.provider, &ctx.settings.cli_path, search)?;
        let _ = settings::ensure_gitignore(&ctx.root);
        let guard = if ctx.settings.confine { Some(write_guard(&ctx)?) } else { None };
        let spec = SpawnSpec {
            env: child_env(&program),
            program,
            args: {
                let mut args = cli::run_args(&ctx.settings, guard.as_deref());
                if ctx.settings.provider == Provider::Prime {
                    args.extend([OsString::from("--cwd"), request.cwd.as_os_str().to_owned()]);
                }
                args
            },
            cwd: request.cwd.clone(),
            stdin: if ctx.settings.provider == Provider::Agy { format!("{}\n", serde_json::json!({"event":"user","message":{"content":request.text}})).into_bytes() } else { request.text.clone().into_bytes() },
        };

        // How the deliverables stand before the run, to tell afterwards what it changed and what to publish.
        let baseline = Baseline::capture(&deliverables::deliverables_dir(&ctx.root, &request.project));

        let run = Arc::new(Run::new(&request, ctx.settings.provider));
        request.run_id = run.id.clone();
        {
            let mut runs = lock(&self.inner.runs);
            if self.inner.held.load(Ordering::SeqCst) {
                return Err(restarting());
            }
            if runs.iter().any(|r| r.is_running() && r.is_for(&request)) {
                return Err(Error::invalid("This prompt is already running in Claude Code"));
            }
            let going = runs.iter().filter(|r| r.is_running()).count();
            if going >= MAX_CONCURRENT {
                return Err(Error::invalid(format!(
                    "{going} runs are already going, which is the most at once. Wait for one to finish, or cancel one."
                )));
            }
            runs.push(run.clone());
            // Only the latest few finished runs are kept for the screen.
            let finished = runs.iter().filter(|r| !r.is_running()).count();
            if finished > KEPT_RUNS {
                let mut excess = finished - KEPT_RUNS;
                runs.retain(|r| {
                    if excess > 0 && !r.is_running() {
                        excess -= 1;
                        false
                    } else {
                        true
                    }
                });
            }
        }

        let view = run.view();
        self.inner.host.emit(&RunEvent { run: view.clone(), chunk: None });
        let worker = {
            let (inner, run) = (self.inner.clone(), run.clone());
            thread::Builder::new().name("claude-run".into()).spawn(move || execute(inner, run, spec, request, ctx, baseline))
        };
        if let Err(e) = worker {
            {
                let mut state = lock(&run.state);
                state.status = Status::Failed;
                state.finished_at = Some(ids::now());
                state.message = Some(format!("Couldn't start a thread for the run: {e}"));
            }
            self.inner.host.emit(&RunEvent { run: run.view(), chunk: None });
        }
        Ok(run.view())
    }

    fn find(&self, id: &str) -> Result<Arc<Run>> {
        lock(&self.inner.runs).iter().find(|r| r.id == id).cloned().ok_or_else(|| Error::not_found("That run is no longer listed"))
    }

    /// Every run still listed, oldest first, with its output so far.
    pub fn list(&self) -> Vec<RunSnapshot> {
        let runs: Vec<Arc<Run>> = lock(&self.inner.runs).clone();
        runs.iter().map(|r| r.snapshot()).collect()
    }

    pub fn snapshot(&self, id: &str) -> Result<RunSnapshot> {
        Ok(self.find(id)?.snapshot())
    }

    /// Stops the run: the process and everything it started. The run ends as cancelled once the process is gone.
    pub fn cancel(&self, id: &str) -> Result<RunView> {
        let run = self.find(id)?;
        if run.is_running() && !run.cancel.swap(true, Ordering::SeqCst) {
            self.inner.host.emit(&RunEvent { run: run.view(), chunk: None });
            // Stopping a process tree can take a moment: not on the caller's thread.
            let process = lock(&run.process).clone();
            if let Some(process) = process.filter(|_| run.provider != Provider::Prime) {
                thread::spawn(move || process.kill_tree());
            }
            // Without a process yet, the worker notices the flag as soon as it has started one.
        }
        Ok(run.view())
    }

    /// Prevent new runs while the app drains its owned sessions for exit.
    pub fn stop_for_exit(&self) {
        {
            let _runs = lock(&self.inner.runs);
            self.inner.held.store(true, Ordering::SeqCst);
        }
        self.cancel_all();
    }

    /// Stops every run. Prime's worker sends cancel/close asynchronously; callers must keep the app alive until it ends.
    pub fn cancel_all(&self) {
        let runs: Vec<Arc<Run>> = lock(&self.inner.runs).clone();
        for run in runs.iter().filter(|r| r.is_running()) {
            run.cancel.store(true, Ordering::SeqCst);
            let process = lock(&run.process).clone();
            if let Some(process) = process.filter(|_| run.provider != Provider::Prime) {
                process.kill_tree();
            }
        }
    }
}

fn restarting() -> Error {
    Error::invalid("Ledger is restarting to install an update. Run the prompt again once it is back.")
}

/// The environment the program is started with: the app's own, plus no colour codes in the output, and the folder the
/// program lives in at the front of the PATH. A GUI app on macOS has a short PATH, and an npm install of Claude Code
/// is a script that needs `node`, which usually sits in the same folder.
pub fn child_env(program: &Path) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = vec![("NO_COLOR".into(), "1".into())];
    if let Some(dir) = program.parent().filter(|d| !d.as_os_str().is_empty()) {
        let existing = std::env::var_os("PATH").unwrap_or_default();
        let paths = std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&existing));
        if let Ok(joined) = std::env::join_paths(paths) {
            env.push(("PATH".into(), joined));
        }
    }
    env
}

/// The guard rails' settings file, rewritten for each run (it is small, and shown in the confirmation).
fn write_guard(ctx: &RunContext) -> Result<PathBuf> {
    let file = config::guard_path(&ctx.root);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    std::fs::write(&file, cli::guard_settings(&ctx.root)).map_err(|e| Error::io(&file, e))?;
    Ok(file)
}

// ---- the worker -------------------------------------------------------------------------------------

fn execute(inner: Arc<Inner>, run: Arc<Run>, spec: SpawnSpec, request: RunRequest, ctx: RunContext, baseline: Baseline) {
    let sink = Arc::new(Sink {
        run: run.clone(),
        host: inner.host.clone(),
        redactor: Redactor::for_vault(&ctx.root),
    });

    if ctx.settings.provider == Provider::Prime {
        execute_prime(&inner, &sink, &spec, &request, &ctx, &baseline);
        return;
    }

    let spawned = match inner.runner.spawn(&spec) {
        Ok(spawned) => spawned,
        Err(e) => {
            let reason = spawn_failure(&e, &spec).replace("Claude Code", ctx.settings.provider.label());
            sink.push(&format!("✗ {reason}\n"));
            finish(&inner, &sink, &request, &ctx, &baseline, Status::Failed, Some(reason), None);
            return;
        }
    };
    *lock(&run.process) = Some(spawned.process.clone());
    if run.cancel.load(Ordering::SeqCst) {
        spawned.process.kill_tree();
    }

    let (done_tx, done_rx) = mpsc::channel::<()>();
    for (stream, is_stdout) in [(spawned.stdout, true), (spawned.stderr, false)] {
        let (sink, done) = (sink.clone(), done_tx.clone());
        let started = thread::Builder::new().name("claude-output".into()).spawn(move || {
            read_stream(stream, is_stdout, &sink);
            let _ = done.send(());
        });
        if started.is_err() {
            let _ = done_tx.send(());
        }
    }
    drop(done_tx);

    let exit = spawned.process.wait();
    // The readers end when the pipes close. A program started by the run could hold them open after Claude Code is
    // gone, so they get a moment to finish, not forever.
    let deadline = Instant::now() + Duration::from_secs(2);
    for _ in 0..2 {
        let _ = done_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    }

    let cancelled = run.cancel.load(Ordering::SeqCst);
    let mut result = lock(&run.state).result.clone();
    if ctx.settings.provider != Provider::Claude && (result.is_none() || !lock(&run.state).terminal) {
        result = Some(ResultInfo { is_error: true, text: "The harness exited without a recognized final response. Check its version and output format.".to_string(), ..ResultInfo::default() });
        lock(&run.state).result = result.clone();
    }
    let (status, message, code) = outcome(cancelled, &exit, result.as_ref(), ctx.settings.confine && run.unknown_option.load(Ordering::SeqCst));
    let message = message.map(|m| m.replace("Claude Code", ctx.settings.provider.label()));
    sink.push(&render::summary(result.as_ref(), run.started.elapsed().as_secs()));
    finish(&inner, &sink, &request, &ctx, &baseline, status, message, code);
}

/// ACP completes a prompt while its facade is still running. Close the owned session before reaping it.
fn execute_prime(inner: &Inner, sink: &Arc<Sink>, spec: &SpawnSpec, request: &RunRequest, ctx: &RunContext, baseline: &Baseline) {
    // Prime's daemon operations can each take 30 seconds. EOF also performs owned-session cleanup.
    const CLEANUP_GRACE: Duration = Duration::from_secs(35);
    const CANCEL_GRACE: Duration = Duration::from_secs(3);
    let interactive = match inner.runner.spawn_interactive(spec) {
        Ok(value) => value,
        Err(e) => {
            let reason = spawn_failure(&e, spec).replace("Claude Code", "Prime Agent");
            sink.push(&format!("✗ {reason}\n"));
            finish(inner, sink, request, ctx, baseline, Status::Failed, Some(reason), None);
            return;
        }
    };
    let runner::InteractiveSpawned { spawned, stdin } = interactive;
    let input = AcpInput::start(stdin);
    let process = spawned.process;
    *lock(&sink.run.process) = Some(process.clone());
    let (out_tx, out_rx) = mpsc::sync_channel(64);
    let reader = thread::Builder::new().name("prime-acp-output".into()).spawn(move || {
        let mut reader = BufReader::new(spawned.stdout);
        let mut line = Vec::new();
        loop {
            let item = match runner::read_line(&mut reader, &mut line, MAX_LINE_BYTES + 1) {
                Ok(true) if line.len() <= MAX_LINE_BYTES => Ok(String::from_utf8_lossy(&line).into_owned()),
                Ok(true) => Err("Prime Agent ACP frame exceeded the size limit".to_string()),
                Ok(false) => break,
                Err(e) => Err(format!("Couldn't read Prime Agent ACP output: {e}")),
            };
            if out_tx.send(item).is_err() { break; }
        }
    });
    let stderr_sink = sink.clone();
    let stderr_reader = thread::Builder::new().name("prime-acp-stderr".into()).spawn(move || read_stream(spawned.stderr, false, &stderr_sink));
    let (exit_tx, exit_rx) = mpsc::channel();
    let waiter = process.clone();
    thread::spawn(move || { let _ = exit_tx.send(waiter.wait()); });

    let mut client = acp::Client::new(spec.cwd.to_str().expect("Prime cwd validated before launch").to_owned(), request.text.clone());
    let mut failure = None;
    let mut input = input;
    let mut result = None;
    let mut cancelling_at = None;
    let mut cancel_sent = false;
    let startup = Instant::now();
    if reader.is_err() || stderr_reader.is_err() || input.is_err() {
        failure = Some("Couldn't start Prime Agent I/O workers".to_string());
    } else if let Err(e) = input.as_mut().map_err(|e| io::Error::other(e.to_string())).and_then(|input| input.send(&client.initialize())) {
        failure = Some(format!("Couldn't initialize Prime Agent ACP: {e}"));
    }
    while failure.is_none() && result.is_none() {
        if let Err(e) = input.as_mut().expect("input started").check() {
            failure = Some(format!("Couldn't write to Prime Agent ACP: {e}"));
            break;
        }
        if sink.run.cancel.load(Ordering::SeqCst) {
            let at = *cancelling_at.get_or_insert_with(Instant::now);
            if let Some(cancel) = client.cancel() {
                if !cancel_sent {
                    if let Err(e) = input.as_mut().map_err(|e| io::Error::other(e.to_string())).and_then(|input| input.send(&cancel)) { failure = Some(format!("Couldn't cancel Prime Agent: {e}")); break; }
                    cancel_sent = true;
                }
            }
            if at.elapsed() >= CANCEL_GRACE { break; }
        }
        if client.session_id.is_none() && startup.elapsed() >= Duration::from_secs(35) {
            failure = Some("Prime Agent ACP initialization timed out".into());
            break;
        }
        match out_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(line)) => match client.receive(&line) {
                Ok(action) => {
                    apply_rendered(sink, action.rendered);
                    // Never start a new prompt after Cancel was pressed during initialization.
                    if let Some(send) = action.send {
                        if !(sink.run.cancel.load(Ordering::SeqCst) && send["method"] == "session/prompt") {
                            if let Err(e) = input.as_mut().map_err(|e| io::Error::other(e.to_string())).and_then(|input| input.send(&send)) { failure = Some(format!("Couldn't write to Prime Agent ACP: {e}")); }
                        }
                    }
                    result = action.result;
                }
                Err(e) => failure = Some(e),
            },
            Ok(Err(e)) => failure = Some(e),
            Err(mpsc::RecvTimeoutError::Disconnected) => failure = Some("Prime Agent exited before completing its ACP prompt".into()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    let mut cleanup_error = None;
    if let Some(close) = client.close() {
        if let Err(e) = input.as_mut().map_err(|e| io::Error::other(e.to_string())).and_then(|input| input.send(&close)) {
            cleanup_error = Some(format!("Couldn't close Prime Agent session: {e}"));
        } else {
            let deadline = Instant::now() + CLEANUP_GRACE;
            loop {
                if let Err(e) = input.as_mut().expect("input started").check() {
                    cleanup_error = Some(format!("Prime Agent session cleanup is unconfirmed: {e}"));
                    break;
                }
                if Instant::now() >= deadline {
                    cleanup_error = Some("Prime Agent session cleanup timed out; closing its input".into());
                    break;
                }
                match out_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(Ok(line)) => match client.receive(&line) {
                        Ok(action) if action.closed => break,
                        Ok(action) => {
                            apply_rendered(sink, action.rendered);
                            if let Some(send) = action.send {
                                if let Err(e) = input.as_mut().expect("input started").send(&send) {
                                    cleanup_error = Some(format!("Prime Agent session cleanup is unconfirmed: {e}"));
                                    break;
                                }
                            }
                        }
                        Err(e) => { cleanup_error = Some(e); break; }
                    },
                    Ok(Err(e)) => { cleanup_error = Some(e); break; }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => { cleanup_error = Some("Prime Agent did not acknowledge session cleanup; closing its input".into()); break; }
                }
            }
        }
    }
    // EOF is scoped to this fresh client-owned session; never shut down the shared Prime daemon.
    drop(input);
    drop(out_rx);
    let exit = match exit_rx.recv_timeout(CLEANUP_GRACE) {
        Ok(exit) => exit,
        Err(_) => {
            process.kill_tree();
            cleanup_error = Some("Prime Agent did not exit after session cleanup; stopped its process tree. Check Prime Agent for remaining work.".into());
            exit_rx.recv_timeout(Duration::from_secs(5)).unwrap_or_else(|_| Err(io::Error::other("Prime Agent did not stop")))
        }
    };
    let cleanup_warning = cleanup_error.map(|e| format!("{e}. Session cleanup is unconfirmed; check Prime Agent for remaining work."));
    if let Some(e) = &cleanup_warning {
        sink.push(&format!("✗ {e}\n"));
        failure.get_or_insert_with(|| e.clone());
    }
    let result = failure.map(|text| ResultInfo { is_error:true, text, ..ResultInfo::default() }).or(result);
    {
        let mut state = lock(&sink.run.state);
        state.terminal = true;
        state.result = result.clone();
    }
    let (status, mut message, code) = outcome(sink.run.cancel.load(Ordering::SeqCst), &exit, result.as_ref(), false);
    if status == Status::Cancelled {
        if let Some(warning) = cleanup_warning {
            message = Some(format!("Stopped at your request. {warning}"));
        }
    }
    sink.push(&render::summary(result.as_ref(), sink.run.started.elapsed().as_secs()));
    finish(inner, sink, request, ctx, baseline, status, message.map(|m| m.replace("Claude Code", "Prime Agent")), code);
}

/// A pipe write can block indefinitely if the facade stops reading. Only this worker owns stdin;
/// the supervisor keeps draining output, checking cancellation, and enforcing write deadlines.
struct AcpInput {
    tx: mpsc::SyncSender<Vec<u8>>,
    done: mpsc::Receiver<io::Result<()>>,
    pending: std::collections::VecDeque<Instant>,
    stop: Arc<AtomicBool>,
}

impl AcpInput {
    const CAPACITY: usize = 8;
    const WRITE_GRACE: Duration = Duration::from_secs(3);

    fn start(mut stdin: Box<dyn Write + Send>) -> io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(Self::CAPACITY);
        let (done_tx, done) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        thread::Builder::new().name("prime-acp-input".into()).spawn(move || {
            while let Ok(frame) = rx.recv() {
                if stopping.load(Ordering::SeqCst) { break; }
                let result = stdin.write_all(&frame).and_then(|()| stdin.flush());
                let failed = result.is_err();
                if done_tx.send(result).is_err() || failed { break; }
            }
            // Dropping stdin sends EOF, invoking the facade's owned-session cleanup.
        })?;
        Ok(Self { tx, done, pending: Default::default(), stop })
    }

    fn send(&mut self, frame: &serde_json::Value) -> io::Result<()> {
        self.check()?;
        if self.pending.len() >= Self::CAPACITY {
            return Err(io::Error::other("Prime Agent ACP input queue is full"));
        }
        let mut bytes = serde_json::to_vec(frame)?;
        if bytes.len() > MAX_LINE_BYTES {
            return Err(io::Error::other("Prime Agent ACP input frame exceeded the size limit"));
        }
        bytes.push(b'\n');
        self.tx.try_send(bytes).map_err(|e| io::Error::other(format!("Prime Agent ACP input unavailable: {e}")))?;
        self.pending.push_back(Instant::now());
        Ok(())
    }

    fn check(&mut self) -> io::Result<()> {
        while !self.pending.is_empty() {
            match self.done.try_recv() {
                Ok(result) => { self.pending.pop_front(); result?; }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Err(io::Error::other("Prime Agent ACP input closed")),
            }
        }
        if self.pending.front().is_some_and(|at| at.elapsed() >= Self::WRITE_GRACE) {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "Prime Agent ACP input write timed out"));
        }
        Ok(())
    }
}

impl Drop for AcpInput {
    fn drop(&mut self) {
        // Do not deliver queued prompt work after cancellation or a transport failure.
        self.stop.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod acp_input_tests {
    use super::*;

    struct BlockedPipe {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }

    impl Write for BlockedPipe {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let _ = self.entered.send(());
            self.release.recv().map_err(|_| io::Error::other("test pipe closed"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    #[test]
    fn blocked_acp_input_does_not_block_supervision_and_is_bounded() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut input = AcpInput::start(Box::new(BlockedPipe { entered: entered_tx, release: release_rx })).unwrap();
        let frame = serde_json::json!({"method":"initialize"});
        input.send(&frame).unwrap();
        entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        for _ in 1..AcpInput::CAPACITY { input.send(&frame).unwrap(); }
        assert!(input.send(&frame).unwrap_err().to_string().contains("queue is full"));
        input.pending[0] = Instant::now() - AcpInput::WRITE_GRACE;
        assert_eq!(input.check().unwrap_err().kind(), io::ErrorKind::TimedOut);
        drop(input);
        release_tx.send(()).unwrap();
        // Once unblocked, the worker must discard the queued frames, not start another prompt.
        assert!(entered_rx.recv_timeout(Duration::from_secs(1)).is_err());
    }

    #[test]
    fn acp_input_rejects_oversized_frames() {
        let mut input = AcpInput::start(Box::new(io::sink())).unwrap();
        let frame = serde_json::json!({"text":"x".repeat(MAX_LINE_BYTES)});
        assert!(input.send(&frame).unwrap_err().to_string().contains("size limit"));
        assert!(input.pending.is_empty());
    }
}

fn apply_rendered(sink: &Sink, rendered: render::Rendered) {
    {
        let mut state = lock(&sink.run.state);
        let room = MAX_STEPS_KEPT.saturating_sub(state.steps.len());
        state.steps.extend(rendered.steps.into_iter().take(room));
    }
    sink.push(&rendered.text);
}

/// Reads one stream to its end, a line at a time, into the run's output.
fn read_stream(stream: Box<dyn Read + Send>, is_stdout: bool, sink: &Sink) {
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    while let Ok(true) = runner::read_line(&mut reader, &mut line, MAX_LINE_BYTES) {
        let text = String::from_utf8_lossy(&line).into_owned();
        if is_stdout {
            let rendered = render::render_for(sink.run.provider, &text);
            {
                let mut state = lock(&sink.run.state);
                state.terminal |= rendered.terminal;
                if let Some(result) = rendered.result {
                    state.result = Some(result);
                }
                if state.model.is_none() {
                    state.model = rendered.model;
                }
                let room = MAX_STEPS_KEPT.saturating_sub(state.steps.len());
                state.steps.extend(rendered.steps.into_iter().take(room));
            }
            sink.push(&rendered.text);
        } else {
            let text = text.trim_end();
            if text.to_ascii_lowercase().contains("unknown option") {
                sink.run.unknown_option.store(true, Ordering::SeqCst);
            }
            if !text.is_empty() {
                sink.push(&format!("stderr: {text}\n"));
            }
        }
    }
}

/// How a run ended, from what the process did and what Claude Code said last.
pub fn outcome(cancelled: bool, exit: &io::Result<Exit>, result: Option<&ResultInfo>, old_cli_hint: bool) -> (Status, Option<String>, Option<i32>) {
    let code = exit.as_ref().ok().and_then(|e| e.code);
    if cancelled {
        return (Status::Cancelled, Some("Stopped at your request".to_string()), code);
    }
    let reason = result.filter(|r| r.is_error).map(|r| r.text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string());
    match exit {
        Err(e) => (Status::Failed, Some(format!("Couldn't wait for Claude Code: {e}")), None),
        Ok(exit) if exit.success() => match reason {
            None => (Status::Finished, None, code),
            Some(reason) if reason.is_empty() => (Status::Failed, Some("Claude Code reported an error".to_string()), code),
            Some(reason) => (Status::Failed, Some(reason), code),
        },
        Ok(exit) => {
            let mut message = format!("Claude Code stopped with {}", exit.describe());
            if let Some(reason) = reason.filter(|r| !r.is_empty()) {
                message.push_str(&format!(": {reason}"));
            }
            if old_cli_hint {
                message.push_str(". The Claude Code installed may be too old for the extra guard rails: update it, or turn them off under Settings, Claude Code.");
            }
            (Status::Failed, Some(message), code)
        }
    }
}

fn spawn_failure(e: &io::Error, spec: &SpawnSpec) -> String {
    let program = spec.program.display();
    match e.kind() {
        io::ErrorKind::NotFound => format!("Claude Code couldn't be started: {program} is gone. Check the path under Settings, Claude Code."),
        io::ErrorKind::PermissionDenied => format!("Claude Code couldn't be started: no permission to run {program}."),
        _ => format!("Claude Code couldn't be started ({program}): {e}"),
    }
}

/// The last step: bring the vault up to date with what the run wrote (publishing and linking the deliverables), and
/// only then say the run is over, so that the screen, refreshing on that news, sees the new files.
#[allow(clippy::too_many_arguments)]
fn finish(inner: &Inner, sink: &Sink, request: &RunRequest, ctx: &RunContext, baseline: &Baseline, status: Status, message: Option<String>, code: Option<i32>) {
    let run = &sink.run;
    let changes: Vec<Change> = baseline.changes(&deliverables::deliverables_dir(&ctx.root, &request.project));
    let vault = inner.host.vault_after_run(request, &changes, status == Status::Finished);
    if let Some(error) = &vault.error {
        sink.push(&format!("✗ After the run: {error}\n"));
    }
    for kept in &vault.kept {
        sink.push(&format!("Edits of yours that no version held were published as {kept} before the run changed the file\n"));
    }
    if !vault.linked.is_empty() {
        sink.push(&format!("Published, and linked to the prompt: {}\n", vault.linked.join(", ")));
    } else if !changes.is_empty() {
        let paths: Vec<&str> = changes.iter().map(Change::path).collect();
        sink.push(&format!("Changed in deliverables/: {}\n", paths.join(", ")));
    }
    // What the note lists: the versions now linked to the prompt, or else the files the run changed.
    let files: Vec<String> = if vault.linked.is_empty() { changes.iter().map(|c| c.path().to_string()).collect() } else { vault.linked.clone() };

    // The note about the run, in its thread: made from what the run did, and kept before the run is called over.
    let finished_at = ids::now();
    let record = {
        let state = lock(&run.state);
        let model = state.model.clone().unwrap_or_else(|| ctx.settings.model.clone());
        records::build(
            request,
            &RunFacts {
                provider: ctx.settings.provider.as_str(),
                id: &run.id,
                status: status.as_str(),
                message: message.as_deref(),
                model: &model,
                permission_mode: ctx.settings.permission_mode.as_str(),
                started_at: &run.started_at,
                finished_at: &finished_at,
                seconds: run.started.elapsed().as_secs(),
                result: state.result.as_ref(),
                steps: &state.steps,
                files: &files,
            },
            &sink.redactor,
        )
    };
    if let Err(e) = inner.host.keep_record(&record) {
        sink.push(&format!("✗ The note about this run couldn't be kept in the thread: {e}\n"));
    }

    {
        let mut state = lock(&run.state);
        state.status = status;
        state.finished_at = Some(finished_at);
        state.exit_code = code;
        state.message = message;
        state.linked = vault.linked.clone();
        state.vault_reloaded = vault.reloaded;
        inner.host.emit(&RunEvent { run: run.view_of(&state), chunk: None });
    }
    *lock(&run.process) = None;
}
