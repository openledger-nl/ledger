//! A development build following its git branch: what is made of git's answers (with a git that only says what it is
//! told to), and the whole thing against real repositories, a remote and two clones, when git is on this computer.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use tempfile::tempdir;

use crate::claude_code::runner::OsRunner;
use crate::devupdates::{self, BranchUpdate, Git, Looked, RepoGit, apply, describe, is_breaking, look, needs_rebuild, parse_commits, parse_counts};
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

const BRANCH: (&str, std::result::Result<&str, &str>) = ("rev-parse --abbrev-ref HEAD", Ok("feat/x"));
const UPSTREAM: (&str, std::result::Result<&str, &str>) = ("rev-parse --abbrev-ref --symbolic-full-name @{upstream}", Ok("origin/feat/x"));

#[test]
fn what_git_prints_is_read_into_counts_and_commits() {
    assert_eq!(parse_counts("2\t3"), Some((2, 3)));
    assert_eq!(parse_counts("0 0\n"), Some((0, 0)));
    for bad in ["", "3", "1 2 3", "a b", "-1 2"] {
        assert_eq!(parse_counts(bad), None, "{bad:?}");
    }

    let commits = parse_commits("a1b2c3d\tfeat(ui): add a thing\n\nd4e5f6a\tfix(sync)!: store the token per vault\nnotacommitline\n");
    assert_eq!(commits.len(), 2);
    assert_eq!((commits[0].hash.as_str(), commits[0].subject.as_str(), commits[0].breaking), ("a1b2c3d", "feat(ui): add a thing", false));
    assert_eq!((commits[1].hash.as_str(), commits[1].breaking), ("d4e5f6a", true));
}

#[test]
fn only_a_bang_before_the_colon_marks_a_commit_as_breaking() {
    for breaking in ["feat!: x", "fix(sync)!: x", "refactor(settings)!: stop importing"] {
        assert!(is_breaking(breaking), "{breaking}");
    }
    for fine in ["feat: add!", "fix(sync): x", "Merge pull request #1 from a/b", "wow! this: works", "no colon at all!"] {
        assert!(!is_breaking(fine), "{fine}");
    }
}

#[test]
fn a_rebuild_is_expected_when_something_under_src_tauri_changed() {
    assert!(needs_rebuild(&["src/routes/+page.svelte".into(), "src-tauri/src/prompts.rs".into()]));
    assert!(needs_rebuild(&["src-tauri/Cargo.lock".into()]));
    assert!(!needs_rebuild(&["src/app.css".into(), "README.md".into(), "package.json".into()]));
    assert!(!needs_rebuild(&[]));
}

// ---- what a look makes of git's answers --------------------------------------------------------------------------

#[test]
fn a_checkout_on_no_branch_or_a_branch_that_follows_nothing_has_nothing_to_follow() {
    let detached = FakeGit::new(vec![("rev-parse --abbrev-ref HEAD", Ok("HEAD"))]);
    assert!(matches!(look(&detached, true).unwrap(), Looked::NotFollowing(why) if why.contains("not on a branch")));
    assert_eq!(detached.asked().len(), 1, "nothing is fetched for a checkout that follows nothing");

    let alone = FakeGit::new(vec![BRANCH, ("rev-parse --abbrev-ref --symbolic-full-name", Err("fatal: no upstream configured for branch 'feat/x'"))]);
    let Looked::NotFollowing(why) = look(&alone, true).unwrap() else { panic!("a branch with no upstream follows nothing") };
    assert!(why.contains("--set-upstream-to=origin/feat/x"), "{why}");

    // Any other failure is a failure: no git, not a repository.
    let broken = FakeGit::new(vec![("rev-parse --abbrev-ref HEAD", Err("fatal: not a git repository"))]);
    assert!(look(&broken, true).unwrap_err().to_string().contains("not a git repository"));
}

#[test]
fn the_remote_is_asked_only_when_a_look_says_so() {
    let answers = || vec![BRANCH, UPSTREAM, ("fetch --quiet", Ok("")), ("rev-list --left-right --count HEAD...@{upstream}", Ok("0\t0"))];
    let quiet = FakeGit::new(answers());
    look(&quiet, false).unwrap();
    assert!(!quiet.asked().iter().any(|c| c.starts_with("fetch")));
    let asking = FakeGit::new(answers());
    look(&asking, true).unwrap();
    assert!(asking.asked().iter().any(|c| c.starts_with("fetch")));
    // A remote that can't be reached makes the look fail, so the person is told when they ask.
    let offline = FakeGit::new(vec![BRANCH, UPSTREAM, ("fetch --quiet", Err("fatal: unable to access 'https://github.com/x/y.git/': Could not resolve host"))]);
    assert!(look(&offline, true).unwrap_err().to_string().contains("Could not resolve host"));
}

#[test]
fn a_branch_that_is_level_or_only_ahead_has_nothing_to_bring_in() {
    for (counts, ahead) in [("0\t0", 0), ("3\t0", 3)] {
        let git = FakeGit::new(vec![BRANCH, UPSTREAM, ("rev-list --left-right --count", Ok(counts))]);
        let looked = look(&git, false).unwrap();
        assert_eq!(looked, Looked::UpToDate { upstream: "origin/feat/x".into(), ahead });
        assert!(!git.asked().iter().any(|c| c.starts_with("log")), "no list is made when there is nothing new");
    }
    assert_eq!(describe(&Looked::UpToDate { upstream: "origin/main".into(), ahead: 0 }), "This build is up to date with origin/main.");
    assert!(describe(&Looked::UpToDate { upstream: "origin/main".into(), ahead: 1 }).contains("1 commit of its own"));
}

#[test]
fn a_branch_with_commits_of_its_own_and_new_ones_is_left_alone() {
    let git = FakeGit::new(vec![BRANCH, UPSTREAM, ("rev-list --left-right --count", Ok("2\t5"))]);
    let looked = look(&git, false).unwrap();
    assert_eq!(looked, Looked::Diverged { upstream: "origin/feat/x".into(), ahead: 2, behind: 5 });
    let said = describe(&looked);
    assert!(said.contains("5 commits") && said.contains("2 commits") && said.contains("Rebase or merge"), "{said}");
}

#[test]
fn new_commits_are_listed_newest_first_with_the_one_the_update_goes_to() {
    let git = FakeGit::new(vec![
        BRANCH,
        UPSTREAM,
        ("rev-list --left-right --count", Ok("0\t3")),
        ("log --format=%h%x09%s --max-count=30 HEAD..@{upstream}", Ok("c3\tfix(sync)!: store the token per vault\nb2\tfeat(ui): add a thing\na1\tdocs: say it")),
        ("rev-parse --short HEAD", Ok("9f9f9f9")),
        ("rev-parse @{upstream}", Ok("c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3")),
        ("status --porcelain", Ok(" M src/app.css")),
    ]);
    let Looked::Behind(update) = look(&git, false).unwrap() else { panic!("three commits behind is an update") };
    assert_eq!((update.branch.as_str(), update.upstream.as_str(), update.head.as_str()), ("feat/x", "origin/feat/x", "9f9f9f9"));
    assert_eq!(update.target, "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3");
    assert_eq!((update.behind, update.more, update.dirty), (3, 0, true));
    assert_eq!(update.commits.iter().map(|c| c.hash.as_str()).collect::<Vec<_>>(), ["c3", "b2", "a1"]);
    assert!(update.commits[0].breaking && !update.commits[1].breaking);
    assert_eq!(describe(&Looked::Behind(update)), "origin/feat/x has 3 new commits this build doesn't.");
}

#[test]
fn a_long_list_is_cut_and_the_rest_counted() {
    let many: &'static str = Box::leak((0..30).map(|i| format!("h{i}\tfeat: n{i}")).collect::<Vec<_>>().join("\n").into_boxed_str());
    let git = FakeGit::new(vec![
        BRANCH,
        UPSTREAM,
        ("rev-list --left-right --count", Ok("0\t42")),
        ("log", Ok(many)),
        ("rev-parse --short HEAD", Ok("a")),
        ("rev-parse @{upstream}", Ok("b")),
        ("status --porcelain", Ok("")),
    ]);
    let Looked::Behind(update) = look(&git, false).unwrap() else { panic!() };
    assert_eq!((update.behind, update.commits.len(), update.more, update.dirty), (42, devupdates::MAX_LISTED, 12, false));
}

#[test]
fn bringing_the_commits_in_goes_to_the_commit_that_was_announced_and_says_if_a_rebuild_follows() {
    let update = BranchUpdate {
        branch: "main".into(),
        upstream: "origin/main".into(),
        head: "aaa".into(),
        target: "deadbeef".into(),
        behind: 2,
        commits: vec![],
        more: 0,
        dirty: false,
    };
    let rust = FakeGit::new(vec![
        ("rev-parse HEAD", Ok("old")),
        ("merge --ff-only --quiet deadbeef", Ok("")),
        ("diff --name-only old deadbeef", Ok("src/lib/api.ts\nsrc-tauri/src/prompts.rs\n")),
        ("rev-parse --short HEAD", Ok("dead")),
    ]);
    let applied = apply(&rust, &update).unwrap();
    assert_eq!((applied.head.as_str(), applied.commits, applied.rebuild), ("dead", 2, true));

    let front = FakeGit::new(vec![
        ("rev-parse HEAD", Ok("old")),
        ("merge --ff-only --quiet deadbeef", Ok("")),
        ("diff --name-only", Ok("src/app.css\nREADME.md")),
        ("rev-parse --short HEAD", Ok("dead")),
    ]);
    assert!(!apply(&front, &update).unwrap().rebuild);

    // Git refusing (a file with changes of the person's own, a branch that moved on) stops everything after it.
    let refused = FakeGit::new(vec![
        ("rev-parse HEAD", Ok("old")),
        ("merge --ff-only", Err("error: Your local changes to the following files would be overwritten by merge: src/app.css")),
    ]);
    let said = apply(&refused, &update).unwrap_err().to_string();
    assert!(said.contains("would be overwritten"), "{said}");
    assert_eq!(refused.asked().len(), 2, "nothing is read or reloaded after a refusal");
}

// ---- real repositories -----------------------------------------------------------------------------------------

/// Runs git for the test's own setup, apart from whatever the person who runs the tests has configured (signing, say).
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git_here() -> bool {
    Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "-m", message]);
}

/// A remote, the checkout the development build runs from, and a team mate's clone to push from.
struct Repos {
    _keep: tempfile::TempDir,
    dev: std::path::PathBuf,
    mate: std::path::PathBuf,
}

fn repos() -> Repos {
    let keep = tempdir().unwrap();
    let root = keep.path();
    let remote = root.join("remote.git");
    fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "--quiet", "--bare", "-b", "main"]);
    let remote_url = remote.to_str().unwrap();
    let clone = |name: &str| {
        git(root, &["clone", "--quiet", remote_url, name]);
        let dir = root.join(name);
        git(&dir, &["checkout", "--quiet", "-B", "main"]);
        dir
    };
    let dev = clone("dev");
    commit(&dev, "README.md", "# Ledger\n", "chore: start");
    git(&dev, &["push", "--quiet", "-u", "origin", "main"]);
    let mate = clone("mate");
    Repos { _keep: keep, dev, mate }
}

fn real(dir: &Path) -> RepoGit<'_> {
    RepoGit { root: dir.to_path_buf(), runner: &OsRunner }
}

#[test]
fn a_real_checkout_hears_of_new_commits_and_fast_forwards_to_them() {
    if !git_here() {
        return;
    }
    let r = repos();
    let git_dev = real(&r.dev);
    assert!(matches!(look(&git_dev, true).unwrap(), Looked::UpToDate { ahead: 0, .. }));

    // A team mate pushes a change to the front end.
    commit(&r.mate, "src/lib/thing.ts", "export const x = 1;\n", "feat(ui): add a thing");
    git(&r.mate, &["push", "--quiet", "origin", "main"]);

    // Not seen until the remote is asked, then seen.
    assert!(matches!(look(&git_dev, false).unwrap(), Looked::UpToDate { .. }), "without a fetch the checkout knows nothing new");
    let Looked::Behind(update) = look(&git_dev, true).unwrap() else { panic!("the new commit should be found") };
    assert_eq!((update.branch.as_str(), update.upstream.as_str(), update.behind, update.dirty), ("main", "origin/main", 1, false));
    assert_eq!(update.commits[0].subject, "feat(ui): add a thing");
    assert_eq!(update.target, git(&r.mate, &["rev-parse", "HEAD"]));

    // Changes of your own that the commits don't touch don't stop it, and are kept.
    fs::write(r.dev.join("README.md"), "# Ledger, edited\n").unwrap();
    let Looked::Behind(update) = look(&git_dev, false).unwrap() else { panic!() };
    assert!(update.dirty);
    let applied = apply(&git_dev, &update).unwrap();
    assert!(!applied.rebuild, "only the front end changed");
    assert_eq!(git(&r.dev, &["rev-parse", "HEAD"]), update.target);
    assert!(r.dev.join("src/lib/thing.ts").exists());
    assert_eq!(fs::read_to_string(r.dev.join("README.md")).unwrap(), "# Ledger, edited\n");

    // Then Rust changes: a rebuild is expected.
    commit(&r.mate, "src-tauri/src/new.rs", "// new\n", "feat(prompts): add a thing in Rust");
    git(&r.mate, &["push", "--quiet", "origin", "main"]);
    let Looked::Behind(update) = look(&git_dev, true).unwrap() else { panic!() };
    assert!(apply(&git_dev, &update).unwrap().rebuild);
    assert!(matches!(look(&git_dev, true).unwrap(), Looked::UpToDate { ahead: 0, .. }));
}

#[test]
fn the_update_goes_to_the_commit_that_was_announced_not_to_what_the_branch_became() {
    if !git_here() {
        return;
    }
    let r = repos();
    commit(&r.mate, "a.txt", "a\n", "feat: a");
    git(&r.mate, &["push", "--quiet", "origin", "main"]);
    let git_dev = real(&r.dev);
    let Looked::Behind(announced) = look(&git_dev, true).unwrap() else { panic!() };
    let first = announced.target.clone();

    // Another commit arrives and is fetched before the person says yes.
    commit(&r.mate, "b.txt", "b\n", "feat: b");
    git(&r.mate, &["push", "--quiet", "origin", "main"]);
    git(&r.dev, &["fetch", "--quiet"]);

    apply(&git_dev, &announced).unwrap();
    assert_eq!(git(&r.dev, &["rev-parse", "HEAD"]), first, "what was announced is what is applied");
    assert!(r.dev.join("a.txt").exists() && !r.dev.join("b.txt").exists());
}

#[test]
fn a_real_branch_with_commits_of_its_own_is_never_rewritten() {
    if !git_here() {
        return;
    }
    let r = repos();
    commit(&r.dev, "mine.txt", "mine\n", "feat: mine");
    commit(&r.mate, "theirs.txt", "theirs\n", "feat: theirs");
    git(&r.mate, &["push", "--quiet", "origin", "main"]);
    let git_dev = real(&r.dev);
    let before = git(&r.dev, &["rev-parse", "HEAD"]);
    let looked = look(&git_dev, true).unwrap();
    assert!(matches!(looked, Looked::Diverged { ahead: 1, behind: 1, .. }), "{looked:?}");
    assert_eq!(git(&r.dev, &["rev-parse", "HEAD"]), before);
}

#[test]
fn a_file_with_changes_the_new_commits_would_overwrite_stops_the_update_and_loses_nothing() {
    if !git_here() {
        return;
    }
    let r = repos();
    commit(&r.mate, "README.md", "# Ledger, by a mate\n", "docs: reword");
    git(&r.mate, &["push", "--quiet", "origin", "main"]);
    let git_dev = real(&r.dev);
    let Looked::Behind(update) = look(&git_dev, true).unwrap() else { panic!() };

    fs::write(r.dev.join("README.md"), "# my unsaved thoughts\n").unwrap();
    let before = git(&r.dev, &["rev-parse", "HEAD"]);
    let said = apply(&git_dev, &update).unwrap_err().to_string();
    assert!(said.contains("README.md") || said.to_lowercase().contains("overwritten"), "{said}");
    assert_eq!(git(&r.dev, &["rev-parse", "HEAD"]), before, "nothing moved");
    assert_eq!(fs::read_to_string(r.dev.join("README.md")).unwrap(), "# my unsaved thoughts\n", "and nothing of the person's was lost");
}

#[test]
fn a_real_branch_with_no_upstream_follows_nothing_and_a_missing_git_says_so() {
    if !git_here() {
        return;
    }
    let r = repos();
    git(&r.dev, &["checkout", "--quiet", "-b", "scratch"]);
    let looked = look(&real(&r.dev), true).unwrap();
    assert!(matches!(looked, Looked::NotFollowing(ref why) if why.contains("--set-upstream-to=origin/scratch")), "{looked:?}");

    // A folder that is no checkout at all.
    let nowhere = tempdir().unwrap();
    assert!(look(&real(nowhere.path()), true).is_err());
}
