//! Starting the program. Everything that touches a real process sits behind two small traits, [`Runner`] and
//! [`Process`], so the rest of the feature can be tested with a fake that never starts anything.
//!
//! The real runner ([`OsRunner`]) starts the program directly from an argument list: there is no shell, and so
//! no string of command text to quote. (An npm install of Claude Code is a `claude.cmd` shim on Windows. Windows
//! can only run those through `cmd.exe`, and Rust does that itself, escaping every argument for it and refusing
//! the ones it can't escape safely. That is safe here because the arguments are a fixed set of flags and two
//! validated values; the prompt is not an argument.) On Windows the child gets `CREATE_NO_WINDOW`, so no console
//! window appears and takes the focus; on macOS and Linux it leads its own process group, which is what lets
//! cancelling reach everything it started.

use std::ffi::OsString;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// How a process is to be started.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Absolute, and the folder the program runs in.
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,
    /// Written to the program's standard input, which is then closed.
    pub stdin: Vec<u8>,
}

/// How a process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<i32>,
    /// The signal that stopped it (macOS and Linux).
    pub signal: Option<i32>,
}

impl Exit {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub fn describe(&self) -> String {
        match (self.code, self.signal) {
            (Some(code), _) => format!("exit code {code}"),
            (None, Some(signal)) => format!("signal {signal}"),
            (None, None) => "no exit code".to_string(),
        }
    }
}

impl From<ExitStatus> for Exit {
    fn from(status: ExitStatus) -> Exit {
        #[cfg(unix)]
        let signal = std::os::unix::process::ExitStatusExt::signal(&status);
        #[cfg(not(unix))]
        let signal = None;
        Exit { code: status.code(), signal }
    }
}

/// A started process: its output streams, and the handle to wait for it or stop it.
pub struct Spawned {
    pub stdout: Box<dyn Read + Send>,
    pub stderr: Box<dyn Read + Send>,
    pub process: Arc<dyn Process>,
}

pub struct InteractiveSpawned {
    pub spawned: Spawned,
    pub stdin: Box<dyn Write + Send>,
}

pub trait Runner: Send + Sync {
    fn spawn(&self, spec: &SpawnSpec) -> io::Result<Spawned>;

    fn spawn_interactive(&self, _spec: &SpawnSpec) -> io::Result<InteractiveSpawned> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "interactive input is not supported by this runner"))
    }
}

pub trait Process: Send + Sync {
    /// Blocks until the process has ended.
    fn wait(&self) -> io::Result<Exit>;
    /// Stops the process and everything it started. Does nothing once it has ended.
    fn kill_tree(&self);
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- the real thing ---------------------------------------------------------------------------------

pub struct OsRunner;

impl Runner for OsRunner {
    fn spawn(&self, spec: &SpawnSpec) -> io::Result<Spawned> {
        let InteractiveSpawned { spawned, mut stdin } = self.spawn_interactive(spec)?;
        let data = spec.stdin.clone();
        let writer = thread::Builder::new().name("claude-stdin".into()).spawn(move || {
            let _ = stdin.write_all(&data);
        });
        if let Err(e) = writer {
            spawned.process.kill_tree();
            let _ = spawned.process.wait();
            return Err(e);
        }
        Ok(spawned)
    }

    fn spawn_interactive(&self, spec: &SpawnSpec) -> io::Result<InteractiveSpawned> {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args).current_dir(&spec.cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        for (name, value) in &spec.env {
            command.env(name, value);
        }
        hide_window(&mut command);
        lead_own_group(&mut command);

        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| io::Error::other("no standard input"))?;
        let stdout = child.stdout.take().ok_or_else(|| io::Error::other("no standard output"))?;
        let stderr = child.stderr.take().ok_or_else(|| io::Error::other("no standard error"))?;
        Ok(InteractiveSpawned {
            spawned: Spawned { stdout: Box::new(stdout), stderr: Box::new(stderr), process: Arc::new(OsProcess::new(child)) },
            stdin: Box::new(stdin),
        })
    }
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// No console window for the child: it would flash up and take the keyboard focus.
fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// The child becomes the leader of a new process group, so that a signal to the group reaches it and all it started.
fn lead_own_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(not(unix))]
    let _ = command;
}

/// How long a process gets to stop after being asked to (macOS and Linux) before it is killed.
const GRACE: Duration = Duration::from_secs(3);

struct OsProcess {
    pid: u32,
    /// Keep the child and its cancellation state under one lock. On Unix an unreaped leader reserves its PID
    /// (and therefore our process-group ID), even if SIGTERM stops it before its descendants.
    state: Mutex<ProcessState>,
}

struct ProcessState {
    child: Child,
    exit: Option<ExitStatus>,
    asked_to_stop: Option<Instant>,
    killed: bool,
}

impl OsProcess {
    fn new(child: Child) -> OsProcess {
        OsProcess {
            pid: child.id(),
            state: Mutex::new(ProcessState { child, exit: None, asked_to_stop: None, killed: false }),
        }
    }
}

impl Process for OsProcess {
    fn wait(&self) -> io::Result<Exit> {
        // Polling (rather than a blocking wait) keeps the lock free for `kill_tree`.
        loop {
            {
                let mut state = lock(&self.state);
                if let Some(status) = state.exit {
                    return Ok(status.into());
                }
                let stopping = state.asked_to_stop.is_some();
                if state.asked_to_stop.is_some_and(|at| at.elapsed() >= GRACE) && !state.killed {
                    stop_tree(self.pid, &mut state.child, true);
                    state.killed = true;
                }
                // Do not reap the Unix leader until the final group signal. Otherwise the leader can exit on
                // SIGTERM, leave resistant descendants behind, and release the group ID for reuse.
                if !(cfg!(unix) && stopping && !state.killed) {
                    if let Some(status) = state.child.try_wait()? {
                        state.exit = Some(status);
                        return Ok(status.into());
                    }
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn kill_tree(&self) {
        let mut state = lock(&self.state);
        if state.exit.is_some() || state.asked_to_stop.is_some() {
            return;
        }
        #[cfg(not(unix))]
        match state.child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                state.exit = Some(status);
                return;
            }
            Err(_) => return,
        }
        state.asked_to_stop = Some(Instant::now());
        stop_tree(self.pid, &mut state.child, cfg!(windows));
    }
}

#[cfg(windows)]
fn stop_tree(pid: u32, child: &mut Child, _force: bool) {
    use std::os::windows::process::CommandExt;
    // `taskkill /T` walks the tree of child processes (claude.cmd starts cmd.exe, which starts node, which starts the
    // commands the model runs), and `/F` ends them: console programs can't be asked politely.
    let program = std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("System32").join("taskkill.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("taskkill.exe"));
    let stopped = Command::new(program)
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|status| status.success());
    if !stopped {
        let _ = child.kill();
    }
}

#[cfg(unix)]
fn stop_tree(pid: u32, child: &mut Child, force: bool) {
    const SIGKILL: i32 = 9;
    const SIGTERM: i32 = 15;
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    // The child leads its own process group (see `lead_own_group`), so the negative id reaches the whole group.
    // SAFETY: `kill` takes two integers and touches no memory.
    let sent = unsafe { kill(-(pid as i32), if force { SIGKILL } else { SIGTERM }) };
    if sent != 0 {
        let _ = child.kill();
    }
}

#[cfg(not(any(unix, windows)))]
fn stop_tree(_pid: u32, child: &mut Child, _force: bool) {
    let _ = child.kill();
}

// ---- reading what a process prints ------------------------------------------------------------------

/// Reads one line (without its line ending) into `line`, keeping at most `cap` bytes of it and skipping the rest, so
/// a single enormous line can't use up the memory. `false` at the end of the stream.
pub fn read_line(reader: &mut impl BufRead, line: &mut Vec<u8>, cap: usize) -> io::Result<bool> {
    line.clear();
    let mut any = false;
    loop {
        let available = match reader.fill_buf() {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            break;
        }
        any = true;
        let (take, found) = match available.iter().position(|&b| b == b'\n') {
            Some(i) => (i, true),
            None => (available.len(), false),
        };
        let room = cap.saturating_sub(line.len());
        line.extend_from_slice(&available[..take.min(room)]);
        let used = if found { take + 1 } else { take };
        reader.consume(used);
        if found {
            break;
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Ok(any)
}

pub struct Captured {
    pub exit: Exit,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Runs a short command to its end and collects what it printed, stopping it if it takes longer than `timeout`.
pub fn capture(runner: &dyn Runner, spec: &SpawnSpec, timeout: Duration) -> io::Result<Captured> {
    const CAP: usize = 64 * 1024;
    let spawned = runner.spawn(spec)?;
    let done = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    {
        let (done, timed_out, process) = (done.clone(), timed_out.clone(), spawned.process.clone());
        thread::spawn(move || {
            let started = Instant::now();
            while started.elapsed() < timeout {
                if done.load(Ordering::SeqCst) {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            if !done.swap(true, Ordering::SeqCst) {
                timed_out.store(true, Ordering::SeqCst);
                process.kill_tree();
            }
        });
    }

    let reader = |mut stream: Box<dyn Read + Send>| {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = (&mut stream).take(CAP as u64).read_to_end(&mut bytes);
            // Whatever is left is read and dropped, so the program never blocks on a full pipe.
            let _ = io::copy(&mut stream, &mut io::sink());
            let _ = tx.send(String::from_utf8_lossy(&bytes).into_owned());
        });
        rx
    };
    let (out, err) = (reader(spawned.stdout), reader(spawned.stderr));
    let exit = spawned.process.wait();
    done.store(true, Ordering::SeqCst);
    let grace = Duration::from_secs(2);
    Ok(Captured {
        exit: exit?,
        stdout: out.recv_timeout(grace).unwrap_or_default(),
        stderr: err.recv_timeout(grace).unwrap_or_default(),
        timed_out: timed_out.load(Ordering::SeqCst),
    })
}
