//! A development build that waits for you: what is made of the runner's state file and of git's answers (with a git that
//! only says what it is told to), and the commits listed from a real repository, which has no remote at all.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use tempfile::tempdir;

use crate::claude_code::runner::OsRunner;
use crate::devupdates::{self, Apply, Build, BuildState, Git, Held, Looked, RepoGit, apply, describe, is_breaking, look, parse_commits, read_held};
use crate::error::{Error, Result};

// ---- a git that says what it is told to ---------------------------------------------------------------------

/// Answers a command by the first entry whose words it starts with; remembers what was asked.
struct FakeGit {
    answers: Vec<(&'static str, std::result::Result<&'static str, &'static str>)>,
    asked: Mutex<Vec<String>>,
}

impl FakeGit {
    fn new(answers: Vec<(&'static str, std::result::Result<&'static str, &'static str>)>) -> FakeGit {
        FakeGit { answers, asked: Mutex::new(Vec::new()) }
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

impl Git for FakeGit {
    fn run(&self, args: &[&str]) -> Result<String> {
        let line = args.join(" ");
        self.asked.lock().unwrap().push(line.clone());
        match self.answers.iter().find(|(start, _)| line.starts_with(start)) {
            Some((_, Ok(out))) => Ok(out.to_string()),
            Some((_, Err(said))) => Err(Error::update(*said)),
            None => panic!("git was asked something the test didn't expect: {line}"),
        }
    }
}

const RUNNING: &str = "1111111111111111111111111111111111111111";
const NOW: &str = "2222222222222222222222222222222222222222";

fn held(frontend: bool, build: BuildState) -> Held {
    Held { running: RUNNING.into(), generation: 4, frontend, build: Build { state: build, error: None }, dependencies: false }
}

fn branch_git(log: &'static str, count: &'static str) -> FakeGit {
    FakeGit::new(vec![
        ("rev-parse --abbrev-ref HEAD", Ok("integration/1.0.0")),
        ("rev-parse --short", Ok("1111111")),
        ("rev-parse HEAD", Ok(NOW)),
        ("rev-list --count", Ok(count)),
        ("log", Ok(log)),
    ])
}

// ---- reading what git says --------------------------------------------------------------------------------

#[test]
fn what_git_prints_is_read_into_commits() {
    let commits = parse_commits("abc1234\tfeat(ui): add a thing\ndef5678\tfix(sync)!: store the token per vault\n\nnot a commit line");
    assert_eq!(commits.len(), 2);
    assert_eq!((commits[0].hash.as_str(), commits[0].subject.as_str(), commits[0].breaking), ("abc1234", "feat(ui): add a thing", false));
    assert!(commits[1].breaking);
}

#[test]
fn only_a_bang_before_the_colon_marks_a_commit_as_breaking() {
    assert!(is_breaking("feat!: drop the old sync"));
    assert!(is_breaking("fix(sync)!: store the token per vault"));
    assert!(!is_breaking("feat(ui): say hello!"));
    assert!(!is_breaking("Merge branch 'x': wow!"));
    assert!(!is_breaking("no colon at all!"));
}

// ---- the runner's state file ----------------------------------------------------------------------------

#[test]
fn the_state_file_the_runner_writes_is_read_and_anything_it_leaves_out_is_nothing() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("state.json");
    // As scripts/dev.mjs writes it.
    fs::write(&file, r#"{"running":"abc","generation":3,"frontend":true,"build":{"state":"failed","error":"error[E0425]: cannot find value `x`"},"dependencies":false,"later":1}"#).unwrap();
    let held = read_held(&file).unwrap();
    assert_eq!(held.running, "abc");
    assert_eq!(held.generation, 3);
    assert!(held.frontend && !held.dependencies);
    assert_eq!(held.build, Build { state: BuildState::Failed, error: Some("error[E0425]: cannot find value `x`".into()) });

    fs::write(&file, "{}").unwrap();
    assert_eq!(read_held(&file).unwrap(), Held::default());
    assert!(!Held::default().waiting());

    fs::write(&file, "{ half a fi").unwrap();
    assert!(read_held(&file).unwrap_err().to_string().contains("couldn't be read"));
}

#[test]
fn the_front_end_alone_reloads_the_window_and_anything_else_restarts_the_app() {
    assert!(!held(false, BuildState::None).waiting());
    for (held, restart) in [
        (held(true, BuildState::None), false),
        (held(false, BuildState::Building), true),
        (held(true, BuildState::Ready), true),
        (Held { dependencies: true, ..held(true, BuildState::None) }, true),
    ] {
        assert!(held.waiting(), "{held:?}");
        assert_eq!(held.restart(), restart, "{held:?}");
    }
    assert_eq!(apply(&held(true, BuildState::None)).unwrap(), Apply::Reload);
    assert_eq!(apply(&held(false, BuildState::Ready)).unwrap(), Apply::Restart);
    // Asked to update while a newer change is still being built: the runner starts it once it is done.
    assert_eq!(apply(&held(false, BuildState::Building)).unwrap(), Apply::Restart);
    let failed = Held { build: Build { state: BuildState::Failed, error: Some("error: expected `;`".into()) }, ..held(false, BuildState::None) };
    let refused = apply(&failed).unwrap_err().to_string();
    assert!(refused.contains("doesn't build") && refused.contains("expected `;`") && refused.contains("keeps running"), "{refused}");
}

#[test]
fn the_variable_the_runner_sets_is_the_one_the_app_reads() {
    // scripts/dev.mjs starts the app with this variable and asks for a restart with this code.
    let script = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/dev.mjs")).unwrap();
    assert!(script.contains(&format!("{}: STATE", devupdates::HOLD_VAR)), "the runner names the state file in {}", devupdates::HOLD_VAR);
    assert!(script.contains(&format!("RESTART_CODE = {}", devupdates::RESTART_CODE)));
}

// ---- a look -----------------------------------------------------------------------------------------------

#[test]
fn with_nothing_held_back_only_the_branch_is_asked_for_and_nothing_is_fetched() {
    let git = branch_git("", "0");
    assert_eq!(look(&git, &held(false, BuildState::None)).unwrap(), Looked::UpToDate { branch: "integration/1.0.0".into() });
    assert_eq!(git.asked(), ["rev-parse --abbrev-ref HEAD"]);
}

#[test]
fn what_is_held_back_comes_with_the_commits_the_local_branch_gained_since_the_running_build() {
    let git = branch_git("ccc3333\tMerge branch 'feat/project-attachments' into integration/1.0.0\nbbb2222\tfeat(uploads)!: list a project's attachments", "2");
    let Looked::Waiting(update) = look(&git, &held(false, BuildState::Ready)).unwrap() else { panic!("something is waiting") };
    assert_eq!((update.branch.as_str(), update.head.as_str(), update.target.as_str()), ("integration/1.0.0", "1111111", NOW));
    assert_eq!((update.behind, update.more, update.generation), (2, 0, 4));
    assert_eq!(update.commits.iter().map(|c| c.hash.as_str()).collect::<Vec<_>>(), ["ccc3333", "bbb2222"]);
    assert!(update.commits[1].breaking);
    assert!(update.restart && update.build == BuildState::Ready);
    let asked = git.asked();
    assert!(asked.contains(&format!("rev-list --count {RUNNING}..HEAD")), "{asked:?}");
    assert!(asked.iter().all(|a| !a.starts_with("fetch") && !a.contains("upstream") && !a.contains("origin")), "only the local branch is looked at: {asked:?}");
    assert_eq!(describe(&Looked::Waiting(update)), "integration/1.0.0 has 2 new commits this build doesn't, ready to update.");
}

#[test]
fn files_changed_without_commits_are_waiting_too() {
    let git = FakeGit::new(vec![("rev-parse --abbrev-ref HEAD", Ok("HEAD")), ("rev-parse --short", Ok("1111111")), ("rev-parse HEAD", Ok(RUNNING))]);
    let Looked::Waiting(update) = look(&git, &held(true, BuildState::Building)).unwrap() else { panic!("something is waiting") };
    assert_eq!((update.behind, update.commits.len()), (0, 0));
    assert!(git.asked().iter().all(|a| !a.starts_with("rev-list") && !a.starts_with("log")), "no commits to list when the checkout is where it was");
    assert_eq!(describe(&Looked::Waiting(update)), "Files in the checkout changed since this build. The new version is being built.");
    assert_eq!(describe(&Looked::UpToDate { branch: "HEAD".into() }), "This build runs what this checkout has.");
    assert_eq!(describe(&Looked::UpToDate { branch: "main".into() }), "This build runs what main has.");
}

#[test]
fn a_long_list_is_cut_and_the_rest_counted() {
    let lines: String = (0..devupdates::MAX_LISTED).map(|i| format!("{i:07}\tchore: step {i}\n")).collect();
    let lines: &'static str = Box::leak(lines.into_boxed_str());
    let git = branch_git(lines, "45");
    let Looked::Waiting(update) = look(&git, &held(true, BuildState::None)).unwrap() else { panic!("something is waiting") };
    assert_eq!((update.behind, update.commits.len(), update.more), (45, devupdates::MAX_LISTED, 15));
    assert!(!update.restart, "only the front end changed");
}

#[test]
fn what_git_says_when_it_fails_is_passed_on() {
    let git = FakeGit::new(vec![("rev-parse --abbrev-ref HEAD", Err("fatal: not a git repository"))]);
    assert_eq!(look(&git, &held(true, BuildState::None)).unwrap_err().to_string(), "fatal: not a git repository");
}

// ---- against a real repository, with no remote ----------------------------------------------------------------

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).env("GIT_TERMINAL_PROMPT", "0").output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git_here() -> bool {
    Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) -> String {
    fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", file]);
    git(dir, &["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "commit.gpgsign=false", "commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

#[test]
fn a_real_checkout_lists_what_was_merged_into_its_branch_since_the_running_build() {
    if !git_here() {
        return;
    }
    let dir = tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "integration/1.0.0"]);
    let running = commit(root, "a.txt", "one", "feat(ui): the build that runs");
    // A feature branch, merged in the way an assistant in another worktree would: the files change in this checkout.
    git(root, &["switch", "-q", "-c", "feat/x"]);
    commit(root, "b.txt", "two", "feat(uploads): list a project's attachments");
    git(root, &["switch", "-q", "integration/1.0.0"]);
    git(root, &["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "commit.gpgsign=false", "merge", "-q", "--no-ff", "-m", "Merge branch 'feat/x' into integration/1.0.0", "feat/x"]);

    let repo = RepoGit { root: root.to_path_buf(), runner: &OsRunner };
    let held = Held { running: running.clone(), generation: 1, frontend: true, ..Held::default() };
    let Looked::Waiting(update) = look(&repo, &held).unwrap() else { panic!("the merge is waiting") };
    assert_eq!(update.branch, "integration/1.0.0");
    assert_eq!(update.head, running[..update.head.len()]);
    assert_eq!(update.target, git(root, &["rev-parse", "HEAD"]));
    assert_eq!(update.behind, 2);
    let subjects: Vec<&str> = update.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["Merge branch 'feat/x' into integration/1.0.0", "feat(uploads): list a project's attachments"]);

    // On a commit rather than a branch, it is said so.
    git(root, &["switch", "-q", "--detach", "HEAD"]);
    assert!(matches!(look(&repo, &held).unwrap(), Looked::Waiting(u) if u.branch == "HEAD"));
    assert_eq!(look(&repo, &Held::default()).unwrap(), Looked::UpToDate { branch: "HEAD".into() });
}
