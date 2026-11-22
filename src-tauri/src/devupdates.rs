//! A development build that waits for you: what changes in the checkout is built, but only shown once you say so.
//!
//! `pnpm run dev:app` (scripts/dev.mjs) runs Ledger from the checkout the person works in, the way `tauri dev` does,
//! and keeps watching its files. Unlike `tauri dev`, it doesn't put a change in front of the running app by itself: a
//! change to the front end doesn't reload the window, and a change to the Rust side is built in the background while
//! the app keeps running the build it started with. What it is holding back it writes to a small file, whose path it
//! gives the app in `LEDGER_DEV_HOLD`. The app reads that file, lists the commits the local branch has gained since the
//! running build (from git, here, without asking any remote), and asks. Only *Update* brings the new version in: the
//! window reloads when only the front end changed, and when the Rust side did the app exits with [`RESTART_CODE`],
//! which tells the runner to start the new build in its place. Quitting and starting `pnpm run dev:app` again does the
//! same.
//!
//! The branch is whatever the checkout has, so commits merged into it (by you, or by an assistant working in another
//! worktree) are what is announced. Nothing is pulled, merged or fetched here: the files are already there.
//!
//! Git sits behind [`Git`], so what is made of its answers can be tested without a repository, and again with a real one.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::claude_code::runner::{Runner, SpawnSpec, capture};
use crate::error::{Error, Result};

/// A look at the branch is quick; this is for a git that hangs.
const TIMEOUT: Duration = Duration::from_secs(30);
/// How many of the new commits are listed; the rest are counted.
pub const MAX_LISTED: usize = 30;
/// The exit code that asks `pnpm run dev:app` to start the new build in place of this one.
pub const RESTART_CODE: i32 = 75;
/// Where `pnpm run dev:app` says what it is holding back.
pub const HOLD_VAR: &str = "LEDGER_DEV_HOLD";

/// One new commit: its short hash, its first line, and whether that line marks it as breaking (`feat!:`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub breaking: bool,
}

// ---- what the runner is holding back ------------------------------------------------------------------------

/// The new Rust build, as the runner has it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildState {
    /// Nothing under `src-tauri/` changed since the running build.
    #[default]
    None,
    /// Something did, and it is being built.
    Building,
    /// The new build is ready to take over.
    Ready,
    /// It doesn't build; the error says why.
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Build {
    pub state: BuildState,
    /// The first error the build gave, when it failed.
    pub error: Option<String>,
}

/// The state file `pnpm run dev:app` keeps (see scripts/dev.mjs).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Held {
    /// The commit the checkout was at when what is running was built.
    pub running: String,
    /// Counts every change the runner has held back, so the same changes are announced once.
    pub generation: u64,
    /// The front end changed since the window loaded.
    pub frontend: bool,
    pub build: Build,
    /// `package.json` or `pnpm-lock.yaml` changed: the runner installs the packages before it restarts.
    pub dependencies: bool,
}

impl Held {
    /// Whether anything is being held back.
    pub fn waiting(&self) -> bool {
        self.frontend || self.dependencies || self.build.state != BuildState::None
    }

    /// Whether bringing it in needs the app to restart, rather than only the window to reload.
    pub fn restart(&self) -> bool {
        self.dependencies || self.build.state != BuildState::None
    }
}

/// The state file, when this build was started by `pnpm run dev:app`. Only a development build looks.
pub fn hold_file() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var_os(HOLD_VAR).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

pub fn read_held(file: &Path) -> Result<Held> {
    let text = std::fs::read_to_string(file).map_err(|e| Error::io(file, e))?;
    serde_json::from_str(&text).map_err(|e| Error::update(format!("pnpm run dev:app left a state file that couldn't be read: {e}")))
}

// ---- git --------------------------------------------------------------------------------------------------

/// The git program, run in the checkout.
pub trait Git {
    /// What git printed, without its trailing line break; an error carrying git's own words when it failed.
    fn run(&self, args: &[&str]) -> Result<String>;
}

/// Git as it is on this computer, run in `root`.
pub struct RepoGit<'a> {
    pub root: PathBuf,
    pub runner: &'a dyn Runner,
}

impl Git for RepoGit<'_> {
    fn run(&self, args: &[&str]) -> Result<String> {
        let spec = SpawnSpec {
            program: PathBuf::from("git"),
            args: args.iter().map(OsString::from).collect(),
            cwd: self.root.clone(),
            // Never wait for a person at a prompt that nobody sees.
            env: vec![("GIT_TERMINAL_PROMPT".into(), "0".into()), ("GCM_INTERACTIVE".into(), "never".into()), ("LC_ALL".into(), "C".into())],
            stdin: Vec::new(),
        };
        let out = capture(self.runner, &spec, TIMEOUT).map_err(|e| Error::update(format!("git couldn't be started: {e}")))?;
        if out.timed_out {
            return Err(Error::update(format!("git {} took too long and was stopped", args.first().copied().unwrap_or_default())));
        }
        if !out.exit.success() {
            let said = out.stderr.lines().chain(out.stdout.lines()).map(str::trim).find(|l| !l.is_empty()).unwrap_or("it gave no reason");
            return Err(Error::update(said.to_string()));
        }
        Ok(out.stdout.trim_end().to_string())
    }
}

/// Where the checkout this build was made from is: the folder above `src-tauri`. Only a development build knows it;
/// a release build doesn't carry the path of the computer it was made on.
#[cfg(debug_assertions)]
pub fn checkout() -> Option<PathBuf> {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(std::path::Path::to_path_buf).filter(|p| p.join(".git").exists())
}

#[cfg(not(debug_assertions))]
pub fn checkout() -> Option<PathBuf> {
    None
}

/// Whether a commit's first line marks it as breaking: a `!` before the colon, as in `feat(sync)!: …`.
pub fn is_breaking(subject: &str) -> bool {
    subject.split_once(':').is_some_and(|(head, _)| head.ends_with('!') && !head.contains(' '))
}

/// The commits of `git log --format=%h%x09%s`, one per line.
pub fn parse_commits(text: &str) -> Vec<Commit> {
    text.lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(hash, subject)| Commit { hash: hash.trim().to_string(), subject: subject.trim().to_string(), breaking: is_breaking(subject) })
        .collect()
}

// ---- a look -----------------------------------------------------------------------------------------------

/// What is waiting for the person: told to the window, and shown in Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchUpdate {
    /// The branch the checkout is on, or `HEAD` when it is on a commit.
    pub branch: String,
    /// The commit the running build was made from (short), and the one the checkout is at now (full).
    pub head: String,
    pub target: String,
    /// Commits the branch gained since; `commits` lists the newest of them, `more` is how many are left out.
    pub behind: usize,
    pub commits: Vec<Commit>,
    pub more: usize,
    /// Which change this is, so the same one is announced once and "Not now" holds until another comes.
    pub generation: u64,
    /// Bringing it in restarts the app (the Rust side or the packages changed), rather than reloading the window.
    pub restart: bool,
    pub build: BuildState,
    pub error: Option<String>,
    pub dependencies: bool,
}

/// What a look found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Looked {
    /// Nothing is being held back: the app runs what the checkout has.
    UpToDate { branch: String },
    Waiting(BranchUpdate),
}

/// Looks at what the runner holds back, and at the commits the checkout gained since the running build.
pub fn look(git: &dyn Git, held: &Held) -> Result<Looked> {
    let branch = git.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
    if !held.waiting() {
        return Ok(Looked::UpToDate { branch });
    }
    let target = git.run(&["rev-parse", "HEAD"])?;
    let since = held.running.trim();
    let (behind, commits) = if since.is_empty() || since == target {
        (0, Vec::new())
    } else {
        let range = format!("{since}..HEAD");
        let count = git.run(&["rev-list", "--count", &range])?;
        let behind = count.trim().parse::<usize>().map_err(|_| Error::update(format!("git gave an answer that wasn't understood: {count}")))?;
        let log = git.run(&["log", "--format=%h%x09%s", &format!("--max-count={MAX_LISTED}"), &range])?;
        (behind, parse_commits(&log))
    };
    let head = if since.is_empty() { String::new() } else { git.run(&["rev-parse", "--short", since])? };
    Ok(Looked::Waiting(BranchUpdate {
        branch,
        head,
        target,
        more: behind.saturating_sub(commits.len()),
        behind,
        commits,
        generation: held.generation,
        restart: held.restart(),
        build: held.build.state.clone(),
        error: held.build.error.clone(),
        dependencies: held.dependencies,
    }))
}

/// What *Update* does, decided from what the runner holds now (which may be newer than what was announced).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    /// Only the front end changed: the window reloads.
    Reload,
    /// The app exits with [`RESTART_CODE`] and the runner starts the new build (once a build still going is done).
    Restart,
}

pub fn apply(held: &Held) -> Result<Apply> {
    if held.build.state == BuildState::Failed {
        let why = held.build.error.as_deref().unwrap_or("see the terminal pnpm run dev:app runs in");
        return Err(Error::update(format!("The new version doesn't build, so this one keeps running: {why}")));
    }
    Ok(if held.restart() { Apply::Restart } else { Apply::Reload })
}

/// What to say about a look, in words for Settings.
pub fn describe(looked: &Looked) -> String {
    match looked {
        Looked::UpToDate { branch } if branch == "HEAD" => "This build runs what this checkout has.".to_string(),
        Looked::UpToDate { branch } => format!("This build runs what {branch} has."),
        Looked::Waiting(update) => {
            let what = if update.behind > 0 {
                format!("{} has {} this build doesn't", branch_name(&update.branch), plural(update.behind, "new commit"))
            } else {
                "Files in the checkout changed since this build".to_string()
            };
            let state = match update.build {
                BuildState::Building => ". The new version is being built.",
                BuildState::Failed => ". The new version doesn't build.",
                _ => ", ready to update.",
            };
            format!("{what}{state}")
        }
    }
}

fn branch_name(branch: &str) -> String {
    if branch == "HEAD" { "this checkout (on a commit, not a branch)".to_string() } else { branch.to_string() }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}
