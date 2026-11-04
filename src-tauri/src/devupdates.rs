//! A development build follows the git branch it runs from, the way a release follows the releases.
//!
//! `tauri dev` runs the checkout the person is working in. When the branch that checkout tracks (its upstream) has
//! commits the checkout doesn't, Ledger says so and asks; only a yes brings them in. They are brought in with a
//! fast-forward, so nothing is merged, rebased or overwritten: a branch with commits of its own that the upstream
//! lacks is left alone, and so is a file with changes of yours that the new commits would touch (git refuses, and
//! says which). What is announced is what is applied: the commit looked at, not whatever the branch has become since.
//!
//! The restart is the development toolchain's: the files that change are watched by `tauri dev`, which rebuilds and
//! restarts the app when Rust changed, and by Vite, which reloads the window when only the front end did. Restarting the
//! program ourselves would make `tauri dev` stop with it. So [`Applied::rebuild`] says whether a rebuild is coming, and
//! the window reloads itself when it isn't.
//!
//! Git sits behind [`Git`], so what is done with its answers can be tested without a repository, and again with a real
//! one.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

use crate::claude_code::runner::{Runner, SpawnSpec, capture};
use crate::error::{Error, Result};

/// A look at the remote or a merge is quick; this is for a stuck connection.
const TIMEOUT: Duration = Duration::from_secs(60);
/// How many of the new commits are listed; the rest are counted.
pub const MAX_LISTED: usize = 30;

/// One new commit: its short hash, its first line, and whether that line marks it as breaking (`feat!:`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub breaking: bool,
}

/// The branch has commits this checkout lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchUpdate {
    pub branch: String,
    /// What it follows, as `origin/main`.
    pub upstream: String,
    /// Where this checkout is now, and the commit the update goes to (the one that was looked at).
    pub head: String,
    pub target: String,
    /// All the commits it lacks; `commits` lists the newest of them, `more` is how many are left out.
    pub behind: usize,
    pub commits: Vec<Commit>,
    pub more: usize,
    /// Tracked files have changes that aren't committed. They are kept: the update stops if it would touch them.
    pub dirty: bool,
}

/// What a look at the branch found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Looked {
    /// There is no branch to follow, and why.
    NotFollowing(String),
    UpToDate {
        upstream: String,
        /// Commits here that the upstream doesn't have yet.
        ahead: usize,
    },
    Behind(BranchUpdate),
    /// The branch and its upstream each have commits the other lacks, so it can't be fast-forwarded.
    Diverged {
        upstream: String,
        ahead: usize,
        behind: usize,
    },
}

/// What bringing the commits in did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// Where the checkout is now, and how many commits that took.
    pub head: String,
    pub commits: usize,
    /// Rust, its manifest or the app's config changed: the toolchain is rebuilding, and restarts the app when done.
    pub rebuild: bool,
}

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

/// The branch and what it follows, or `None` (with the reason) when it follows nothing.
fn following(git: &dyn Git) -> Result<std::result::Result<(String, String), String>> {
    let branch = git.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
    if branch == "HEAD" {
        return Ok(Err("The checkout is on a commit, not on a branch, so there is nothing to follow.".to_string()));
    }
    match git.run(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]) {
        Ok(upstream) => Ok(Ok((branch, upstream))),
        Err(e) if e.to_string().contains("no upstream") => {
            Ok(Err(format!("The branch {branch} follows no remote branch. Set one with `git branch --set-upstream-to=origin/{branch}`.")))
        }
        Err(e) => Err(e),
    }
}

/// The two counts of `git rev-list --left-right --count HEAD...@{upstream}`: commits only here, commits only there.
pub fn parse_counts(text: &str) -> Option<(usize, usize)> {
    let mut numbers = text.split_whitespace().map(|n| n.parse::<usize>());
    match (numbers.next(), numbers.next(), numbers.next()) {
        (Some(Ok(ahead)), Some(Ok(behind)), None) => Some((ahead, behind)),
        _ => None,
    }
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

/// Looks at the branch, after asking the remote for what is new when `fetch` is set.
pub fn look(git: &dyn Git, fetch: bool) -> Result<Looked> {
    let (branch, upstream) = match following(git)? {
        Ok(found) => found,
        Err(why) => return Ok(Looked::NotFollowing(why)),
    };
    if fetch {
        git.run(&["fetch", "--quiet"])?;
    }
    let counts = git.run(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])?;
    let (ahead, behind) = parse_counts(&counts).ok_or_else(|| Error::update(format!("git gave an answer that wasn't understood: {counts}")))?;
    if behind == 0 {
        return Ok(Looked::UpToDate { upstream, ahead });
    }
    if ahead > 0 {
        return Ok(Looked::Diverged { upstream, ahead, behind });
    }
    let log = git.run(&["log", "--format=%h%x09%s", &format!("--max-count={}", MAX_LISTED), "HEAD..@{upstream}"])?;
    let commits = parse_commits(&log);
    Ok(Looked::Behind(BranchUpdate {
        head: git.run(&["rev-parse", "--short", "HEAD"])?,
        target: git.run(&["rev-parse", "@{upstream}"])?,
        more: behind.saturating_sub(commits.len()),
        dirty: !git.run(&["status", "--porcelain", "--untracked-files=no"])?.is_empty(),
        branch,
        upstream,
        behind,
        commits,
    }))
}

/// Whether these changed paths (relative to the checkout) make the toolchain rebuild the app.
pub fn needs_rebuild(changed: &[String]) -> bool {
    changed.iter().any(|path| path.starts_with("src-tauri/"))
}

/// Brings the commits in: a fast-forward to the commit that was announced. Git refuses, with its reason, when the branch has
/// moved on in a way that isn't one, or when a file with uncommitted changes would be overwritten; then nothing has changed.
pub fn apply(git: &dyn Git, update: &BranchUpdate) -> Result<Applied> {
    let before = git.run(&["rev-parse", "HEAD"])?;
    git.run(&["merge", "--ff-only", "--quiet", &update.target])?;
    let changed = git.run(&["diff", "--name-only", &before, &update.target])?;
    let changed: Vec<String> = changed.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    Ok(Applied { head: git.run(&["rev-parse", "--short", "HEAD"])?, commits: update.behind, rebuild: needs_rebuild(&changed) })
}

/// What to say about a look, in words for Settings.
pub fn describe(looked: &Looked) -> String {
    match looked {
        Looked::NotFollowing(why) => why.clone(),
        Looked::UpToDate { upstream, ahead: 0 } => format!("This build is up to date with {upstream}."),
        Looked::UpToDate { upstream, ahead } => {
            format!("This build is up to date with {upstream}, and has {} of its own that {upstream} doesn't.", plural(*ahead, "commit"))
        }
        Looked::Behind(update) => format!("{} has {} this build doesn't.", update.upstream, plural(update.behind, "new commit")),
        Looked::Diverged { upstream, ahead, behind } => format!(
            "{upstream} has {} this build lacks, but this build has {} that {upstream} lacks, so it can't be brought up by itself. Rebase or merge in a terminal.",
            plural(*behind, "commit"),
            plural(*ahead, "commit")
        ),
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}
