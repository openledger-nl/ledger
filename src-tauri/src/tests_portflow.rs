//! Portflow against a fake one: no network, no real session. The fake holds a small portfolio (outcomes, people, a
//! semester's collection with two evidence items, versions with files, comments with a reply) and counts every call,
//! so the tests can say exactly what was asked, and that nothing was asked at all while the switch is off.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use tempfile::{TempDir, tempdir};

use crate::models::{FeedbackFrom, SemesterDraft};
use crate::portflow_api::*;
use crate::portflow_auth::Session;
use crate::portflow_config::{self as config, Portfolio};
use crate::portflow_epsilon::{self as epsilon, parse_sub_goal};
use crate::portflow_store as store;
use crate::portflow_sync::{self, CoreLedger, Ledger};
use crate::state::Core;
use crate::{feedback, frontmatter, projects, semesters, threads};

const P: i64 = 22497;
const ME: i64 = 7;

// ---- the gate and the client, with a fake wire --------------------------------------------------------

#[derive(Default)]
struct FakeWire {
    sent: Mutex<Vec<(String, String)>>,
    /// The body of each request sent, in the same order.
    bodies: Mutex<Vec<Option<serde_json::Value>>>,
    replies: Mutex<BTreeMap<String, Vec<Reply>>>,
}

impl FakeWire {
    fn reply(&self, path: &str, status: u16, body: &str) {
        self.replies.lock().unwrap().entry(path.to_string()).or_default().push(Reply { status, body: body.into() });
    }
    fn sent(&self) -> Vec<(String, String)> {
        self.sent.lock().unwrap().clone()
    }
}

impl Wire for Arc<FakeWire> {
    fn send(&self, request: &Request) -> std::result::Result<Reply, String> {
        let page = request.query.iter().find(|(k, _)| k == "page").map(|(_, v)| v.clone());
        self.sent.lock().unwrap().push((request.method.to_string(), format!("{}{}", request.path, page.map(|p| format!("?page={p}")).unwrap_or_default())));
        self.bodies.lock().unwrap().push(request.body.clone());
        let mut replies = self.replies.lock().unwrap();
        let queue = replies.get_mut(&request.path);
        match queue {
            Some(q) if q.len() > 1 => Ok(q.remove(0)),
            Some(q) if q.len() == 1 => Ok(q[0].clone()),
            _ => Ok(Reply { status: 404, body: "{}".into() }),
        }
    }
}

fn gate(enabled: bool, write: bool, writes: &[&str]) -> Gate {
    Gate { enabled, write, writes: writes.iter().map(|w| w.to_string()).collect() }
}

fn client(wire: &Arc<FakeWire>, gate: Gate) -> HttpPortflow<Arc<FakeWire>> {
    HttpPortflow::with_wire(wire.clone(), gate, Arc::new(|_| {}))
}

fn post(path: &str) -> Request {
    Request { method: "POST", path: path.into(), query: vec![], body: None }
}

#[test]
fn switched_off_nothing_is_sent() {
    let wire = Arc::new(FakeWire::default());
    wire.reply("/users/current", 200, r#"{"id": 7, "name": "Me"}"#);
    let off = client(&wire, gate(false, true, &["upload-versions"]));
    assert!(matches!(off.current_user(), Err(PortflowError::Off)));
    assert!(matches!(off.send(&post("/direct-uploads")), Err(PortflowError::Off)));
    assert!(wire.sent().is_empty());
    // And the real client can't even be made while off.
    let session = Credentials { token: "a.b.c".into(), csrf: None };
    assert!(matches!(HttpPortflow::new(&session, gate(false, false, &[])), Err(PortflowError::Off)));
}

#[test]
fn read_only_sends_no_write() {
    let wire = Arc::new(FakeWire::default());
    wire.reply("/users/current", 200, r#"{"id": 7, "name": "Me"}"#);
    wire.reply("/mentions", 200, "[]");
    let read_only = client(&wire, gate(true, false, &["upload-versions"]));
    assert_eq!(read_only.current_user().unwrap().id, 7);
    for path in [
        "/direct-uploads",
        &format!("/portfolios/{P}/evidence/1/versions"),
        &format!("/portfolios/{P}/collections/2/evidence"),
        &format!("/portfolios/{P}/comments/3/children"),
        &format!("/portfolios/{P}/collections/2/feedback-requests"),
        "/something-new",
    ] {
        assert!(matches!(read_only.send(&post(path)), Err(PortflowError::NotAllowed(_))), "{path} went through");
    }
    let put = Request { method: "PUT", path: format!("/portfolios/{P}/collections/2"), query: vec![], body: None };
    assert!(matches!(read_only.send(&put), Err(PortflowError::NotAllowed(_))));
    // The one read Portflow makes with a POST is a read.
    assert!(read_only.send(&post("/mentions")).is_ok());
    assert!(wire.sent().iter().all(|(method, path)| method == "GET" || path == "/mentions"), "{:?}", wire.sent());
}

#[test]
fn writes_go_through_only_when_their_kind_is_on() {
    let wire = Arc::new(FakeWire::default());
    wire.reply("/direct-uploads", 200, "{}");
    let some = client(&wire, gate(true, true, &["upload-versions"]));
    assert!(some.send(&post("/direct-uploads")).is_ok());
    assert!(matches!(some.send(&post(&format!("/portfolios/{P}/comments/3/children"))), Err(PortflowError::NotAllowed(_))));
    // A write this version doesn't know is never allowed, whatever is on.
    let all = client(&wire, gate(true, true, &["upload-versions", "create-evidence", "guide-description", "link-outcomes", "reply-feedback", "ask-feedback", "confirm-feedback"]));
    assert!(matches!(all.send(&post("/something-new")), Err(PortflowError::NotAllowed(_))));
}

#[test]
fn lists_are_read_page_by_page_and_401_means_signed_out() {
    let wire = Arc::new(FakeWire::default());
    let page: Vec<String> = (0..50).map(|i| format!(r#"{{"id": {i}, "name": "c{i}"}}"#)).collect();
    wire.reply(&format!("/portfolios/{P}/collections"), 200, &format!("[{}]", page.join(",")));
    wire.reply(&format!("/portfolios/{P}/collections"), 200, r#"[{"id": 99, "name": "last", "unknown_field": true}]"#);
    let c = client(&wire, gate(true, false, &[]));
    let all = c.collections(P).unwrap();
    assert_eq!(all.len(), 51);
    assert_eq!(all[50].name, "last");
    assert_eq!(wire.sent().len(), 2);

    wire.reply("/users/current", 401, "{}");
    assert!(matches!(c.current_user(), Err(PortflowError::SignedOut)));
    // Something that isn't a list where one is expected: Portflow changed, and it says so.
    wire.reply(&format!("/portfolios/{P}/shares"), 200, r#"{"oops": 1}"#);
    assert!(matches!(c.shares(P), Err(PortflowError::Changed(_))));
}

#[test]
fn trouble_is_retried_then_reported() {
    let wire = Arc::new(FakeWire::default());
    wire.reply("/users/current", 503, "busy");
    wire.reply("/users/current", 200, r#"{"id": 7, "name": "Me"}"#);
    let c = client(&wire, gate(true, false, &[]));
    assert_eq!(c.current_user().unwrap().id, 7);
    assert_eq!(wire.sent().len(), 2);
}

// ---- a fake Portflow ----------------------------------------------------------------------------------

#[derive(Default)]
struct World {
    goals: Vec<(Goal, Vec<SubGoal>)>,
    shares: Vec<Share>,
    collections: Vec<Collection>,
    collection_goals: Vec<CollectionGoal>,
    evidence: Vec<Evidence>,
    evidence_goals: BTreeMap<i64, Vec<i64>>,
    versions: BTreeMap<i64, Vec<Version>>,
    comments: BTreeMap<i64, Vec<Comment>>,
    children: BTreeMap<i64, Vec<Comment>>,
}

struct FakePortflow {
    world: Mutex<World>,
    calls: AtomicUsize,
    /// Every collection description written: (collection, HTML).
    descriptions: Mutex<Vec<(i64, String)>>,
    /// Every signed address downloaded, in order.
    downloads: Mutex<Vec<String>>,
    /// Files whose download fails.
    broken: Mutex<BTreeSet<i64>>,
    /// Every evidence description written: (evidence item, HTML).
    evidence_descriptions: Mutex<Vec<(i64, String)>>,
}

/// What the fake's file `id` holds.
fn bytes_of(id: i64) -> Vec<u8> {
    format!("%PDF the bytes of file {id}").into_bytes()
}

fn person(id: i64, name: &str, role: &str) -> Person {
    Person { id, name: name.into(), current_role: Some(role.into()), pronouns: None }
}

fn file(id: i64, name: &str) -> VersionFile {
    VersionFile { id, display_name: None, file: FileInfo { file_name: name.into(), content_type: None } }
}

fn version(id: i64, evidence: i64, number: i64, kind: &str, files: Vec<VersionFile>) -> Version {
    Version {
        id,
        evidence_id: evidence,
        version_number: number,
        version_type: Some(kind.into()),
        description: Some(format!("<p>Version <strong>{number}</strong></p>")),
        created_at: Some(format!("2026-10-0{number}T10:00:00Z")),
        co_created: false,
        files,
    }
}

fn comment(id: i64, parent: Option<i64>, author: Person, text: &str, version: i64, children: i64) -> Comment {
    Comment {
        id,
        parent_id: parent,
        text: Some(text.into()),
        comment_type: Some(if parent.is_some() { "comment" } else { "feedback" }.into()),
        created_at: Some(format!("2026-10-1{}T09:00:00Z", id % 10)),
        updated_at: None,
        created_by_role: author.current_role.clone(),
        created_by: author,
        child_count: children,
        hidden: false,
        evidence_version: Some(VersionRef { version_number: version }),
        feedback_request: None,
    }
}

/// The template import of the fall semester: its section and its goals share it.
fn fall_import() -> Import {
    Import { id: Some(34), imported_at: Some("2026-09-21T11:42:36.215Z".into()), template_name: Some("OL-BCDFG-FALL26".into()) }
}

impl FakePortflow {
    fn new() -> FakePortflow {
        let goal = |id, name: &str, position| Goal {
            id,
            name: name.into(),
            nickname: None,
            position,
            status: Some("not_yet_started".into()),
            explanation: Some(format!("<p>About {name}</p>")),
            goal_set: Some(Named { id: 1, name: "OL-BCDFG".into() }),
            last_import: Some(fall_import()),
            ..Goal::default()
        };
        let sub = |id, goal_id, name: &str, position| SubGoal {
            id,
            name: name.into(),
            nickname: None,
            description: Some(format!("<p>{name} level</p>")),
            goal_id,
            position,
            evidence_count: if id == 11 { 2 } else { 0 },
        };
        let teacher = person(100, "Coach Teacher", "teacher");
        let world = World {
            goals: vec![
                (goal(10, "Analysis", 1), vec![sub(11, 10, "Analysis-U1", 1), sub(12, 10, "Analysis-I1", 2)]),
                (goal(20, "Design", 2), vec![sub(21, 20, "Design-U1", 1)]),
            ],
            shares: vec![
                Share { id: 1, share_type: Some("portfolio".into()), expiration_date: Some("2099-01-31".into()), feedback_restricted: false, invitee: person(101, "Peer Student", "student") },
                Share { id: 2, share_type: Some("portfolio".into()), expiration_date: Some("2099-01-31".into()), feedback_restricted: false, invitee: teacher.clone() },
            ],
            collections: vec![Collection { id: 500, name: "Speed edit".into(), subtitle: Some("Semester project".into()), description: Some("<p>The <em>plan</em>.</p>".into()), section_id: Some(9), ..Collection::default() }],
            collection_goals: vec![CollectionGoal { id: 1, position: 1, goal: goal(10, "Analysis", 1) }],
            evidence: vec![
                Evidence { id: 700, name: "Project plan".into(), latest_version_type: Some("file".into()), versions_count: 2, created_at: Some("2026-09-01T10:00:00Z".into()), ..Evidence::default() },
                Evidence { id: 800, name: "Research".into(), latest_version_type: Some("file".into()), versions_count: 1, created_at: Some("2026-09-02T10:00:00Z".into()), ..Evidence::default() },
            ],
            evidence_goals: BTreeMap::from([(700, vec![10, 11]), (800, vec![21])]),
            versions: BTreeMap::from([
                (700, vec![version(7002, 700, 2, "file", vec![file(1, "plan.pdf"), file(2, "sources.zip")]), version(7001, 700, 1, "free_text", vec![])]),
                (800, vec![version(8001, 800, 1, "file", vec![file(3, "research.pdf")])]),
            ]),
            comments: BTreeMap::from([(700, vec![comment(9001, None, teacher.clone(), "<p>Good start, add <strong>risks</strong>.</p>", 2, 1)])]),
            children: BTreeMap::from([(9001, vec![comment(9002, Some(9001), person(ME, "Me Student", "student"), "<p>Done!</p>", 2, 0)])]),
        };
        FakePortflow { world: Mutex::new(world), calls: AtomicUsize::new(0), descriptions: Mutex::new(vec![]), downloads: Mutex::new(vec![]), broken: Mutex::new(BTreeSet::new()), evidence_descriptions: Mutex::new(vec![]) }
    }

    fn call(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

impl Portflow for FakePortflow {
    fn current_user(&self) -> PResult<Person> {
        self.call();
        Ok(person(ME, "Me Student", "student"))
    }
    fn sections(&self, _: i64) -> PResult<Vec<Section>> {
        self.call();
        Ok(vec![Section { id: 9, name: "Semester 3 – Fall 26".into(), position: 1, last_import: Some(fall_import()) }])
    }
    fn collections(&self, _: i64) -> PResult<Vec<Collection>> {
        self.call();
        Ok(self.world.lock().unwrap().collections.clone())
    }
    fn collection_goals(&self, _: i64, _: i64) -> PResult<Vec<CollectionGoal>> {
        self.call();
        Ok(self.world.lock().unwrap().collection_goals.clone())
    }
    fn collection_evidence(&self, _: i64, _: i64) -> PResult<Vec<CollectionEvidence>> {
        self.call();
        Ok(self.world.lock().unwrap().evidence.iter().enumerate().map(|(i, e)| CollectionEvidence { id: i as i64, position: i as i64, evidence: e.clone() }).collect())
    }
    fn goal_sets(&self, _: i64) -> PResult<Vec<GoalSet>> {
        self.call();
        Ok(vec![GoalSet { id: 1, name: "OL-BCDFG".into(), goal_ids: vec![10, 20] }])
    }
    fn goals(&self, _: i64) -> PResult<Vec<Goal>> {
        self.call();
        Ok(self.world.lock().unwrap().goals.iter().map(|(g, _)| g.clone()).collect())
    }
    fn sub_goals(&self, _: i64, goal: i64) -> PResult<Vec<SubGoal>> {
        self.call();
        Ok(self.world.lock().unwrap().goals.iter().find(|(g, _)| g.id == goal).map(|(_, s)| s.clone()).unwrap_or_default())
    }
    fn shares(&self, _: i64) -> PResult<Vec<Share>> {
        self.call();
        Ok(self.world.lock().unwrap().shares.clone())
    }
    fn evidence_sub_goals(&self, _: i64, e: i64) -> PResult<Vec<SubGoal>> {
        self.call();
        let world = self.world.lock().unwrap();
        let ids = world.evidence_goals.get(&e).cloned().unwrap_or_default();
        Ok(world.goals.iter().flat_map(|(_, s)| s.clone()).filter(|s| ids.contains(&s.id)).collect())
    }
    fn evidence_goals(&self, _: i64, e: i64) -> PResult<Vec<Goal>> {
        self.call();
        let world = self.world.lock().unwrap();
        let ids = world.evidence_goals.get(&e).cloned().unwrap_or_default();
        Ok(world.goals.iter().map(|(g, _)| g.clone()).filter(|g| ids.contains(&g.id)).collect())
    }
    fn versions(&self, _: i64, e: i64) -> PResult<Vec<Version>> {
        self.call();
        Ok(self.world.lock().unwrap().versions.get(&e).cloned().unwrap_or_default().into_iter().map(|v| Version { files: vec![], ..v }).collect())
    }
    fn version(&self, _: i64, v: i64) -> PResult<Version> {
        self.call();
        self.world.lock().unwrap().versions.values().flatten().find(|x| x.id == v).cloned().ok_or_else(|| PortflowError::message("no such version"))
    }
    fn evidence_comments(&self, _: i64, e: i64) -> PResult<Vec<Comment>> {
        self.call();
        Ok(self.world.lock().unwrap().comments.get(&e).cloned().unwrap_or_default())
    }
    fn comment_children(&self, _: i64, c: i64) -> PResult<Vec<Comment>> {
        self.call();
        Ok(self.world.lock().unwrap().children.get(&c).cloned().unwrap_or_default())
    }
    fn file_url(&self, _: i64, f: i64) -> PResult<String> {
        self.call();
        Ok(format!("https://s3.example/files/{f}?signature=abc"))
    }
    fn download(&self, url: &str) -> PResult<Vec<u8>> {
        self.downloads.lock().unwrap().push(url.to_string());
        let id: i64 = url.trim_start_matches("https://s3.example/files/").split('?').next().unwrap().parse().unwrap();
        if self.broken.lock().unwrap().contains(&id) {
            return Err(PortflowError::message("The file's address answered 503"));
        }
        Ok(bytes_of(id))
    }
    fn set_collection_description(&self, _: i64, c: i64, html: &str) -> PResult<Option<String>> {
        self.call();
        self.descriptions.lock().unwrap().push((c, html.to_string()));
        if let Some(found) = self.world.lock().unwrap().collections.iter_mut().find(|x| x.id == c) {
            found.description = Some(html.to_string());
        }
        Ok(Some(html.to_string()))
    }
    fn evidence_item(&self, _: i64, e: i64) -> PResult<Evidence> {
        self.call();
        self.world.lock().unwrap().evidence.iter().find(|x| x.id == e).cloned().ok_or_else(|| PortflowError::message("no such evidence item"))
    }
    fn set_evidence_description(&self, _: i64, e: i64, html: &str) -> PResult<Option<String>> {
        self.call();
        self.evidence_descriptions.lock().unwrap().push((e, html.to_string()));
        if let Some(found) = self.world.lock().unwrap().evidence.iter_mut().find(|x| x.id == e) {
            found.description = Some(html.to_string());
        }
        Ok(Some(html.to_string()))
    }
}

// ---- a machine ----------------------------------------------------------------------------------------

struct Machine {
    dir: TempDir,
    _config: TempDir,
    core: Arc<Mutex<Core>>,
    ledger: CoreLedger,
}

impl Machine {
    fn new() -> Machine {
        let dir = tempdir().unwrap();
        Machine::at(dir)
    }

    fn at(dir: TempDir) -> Machine {
        Machine::open(dir, tempdir().unwrap())
    }

    /// The app started again on the same machine: the same vault, and the same config folder with this machine's
    /// settings and bookkeeping for it.
    fn restart(self) -> Machine {
        let Machine { dir, _config: config, .. } = self;
        Machine::open(dir, config)
    }

    fn open(dir: TempDir, config: TempDir) -> Machine {
        let core = Arc::new(Mutex::new(Core::open_at(dir.path().to_path_buf(), config.path().to_path_buf(), false).unwrap()));
        let ledger = CoreLedger { core: core.clone(), changed: Arc::new(|| {}), vault: dir.path().to_path_buf() };
        Machine { dir, _config: config, core, ledger }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn read<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&self.core.lock().unwrap().conn)
    }

    fn edit<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let mut guard = self.core.lock().unwrap();
        let core = &mut *guard;
        let out = f(&core.conn);
        core.vault.persist(&core.conn).unwrap();
        out
    }

    fn reload(&self) {
        self.core.lock().unwrap().reload().unwrap();
    }

    fn switch(&self, on: bool) {
        config::update_settings(self.root(), |s| {
            s.enabled = on;
            s.portfolio = Some(Portfolio { id: P, user_id: ME, user_name: "Me Student".into() });
        })
        .unwrap();
        self.reload();
    }

    /// A project linked to the fake's collection for a semester.
    fn linked(&self) -> String {
        self.edit(|c| {
            projects::create(c, "Speed edit").unwrap();
            let semester = semesters::create(
                c,
                &SemesterDraft { name: "Fall 26".into(), start: "2026-09-01".into(), end: "2027-01-31".into(), sprint_weeks: 3, notes: String::new(), sprints: vec![], vacations: vec![] },
            )
            .unwrap();
            store::link_collection(c, "speed-edit", &semester.id, Some(500)).unwrap();
            semester.id
        })
    }

    fn settings(&self) -> config::PortflowSettings {
        config::read_settings(self.root())
    }

    fn files(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    out.insert(rel, fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(self.root(), self.root(), &mut out);
        out
    }
}

fn outcome_count(c: &Connection) -> i64 {
    c.query_row("SELECT COUNT(*) FROM outcomes", [], |r| r.get(0)).unwrap()
}

// ---- the round ----------------------------------------------------------------------------------------

#[test]
fn a_round_mirrors_outcomes_people_and_the_linked_collection() {
    let m = Machine::new();
    m.switch(true);
    let semester = m.linked();
    let fake = FakePortflow::new();
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((report.outcomes, report.people, report.collections, report.evidence), (2, 2, 1, 2));
    assert_eq!(report.unplaced, 2);
    assert_eq!(report.new_versions, 3);
    assert_eq!((report.downloaded, report.in_portflow_only), (3, 0), "a linked project without facts is a study one: its evidence is downloaded");
    assert!(report.notes.is_empty(), "{:?}", report.notes);

    let epsilon = m.read(|c| epsilon::epsilon(c, None, "2026-11-04", ME)).unwrap();
    assert_eq!(epsilon.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Analysis", "Design"]);
    assert_eq!(epsilon.cells.iter().filter(|c| c.outcome == 10).count(), 2);
    let analysis_u = epsilon.cells.iter().find(|c| c.outcome == 10 && c.layer == "U").unwrap();
    assert_eq!(analysis_u.levels[0].evidence[0].name, "Project plan");
    let people = m.read(store::people).unwrap();
    assert_eq!(people.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Coach Teacher", "Peer Student"], "teachers first");

    let collections = m.read(|c| store::project_collections(c, "speed-edit")).unwrap();
    assert_eq!(collections[0].name, "Speed edit");
    assert_eq!(collections[0].section, "Semester 3 – Fall 26");
    // Its description isn't mirrored: it is the reading guide (a linked project without facts counts as a study project).
    assert!(readme_of(&m).contains("plan"), "{}", readme_of(&m));
    assert_eq!(collections[0].outcomes[0].name, "Analysis");

    // The files: the mirror, the collection, the evidence items, and their files as deliverables.
    let root = m.root();
    assert!(root.join("shared/portflow/outcomes/10-analysis.md").is_file());
    assert!(root.join("shared/portflow/people.md").is_file());
    assert!(root.join(format!("projects/speed-edit/portflow/{semester}.md")).is_file());
    let plan = fs::read_to_string(root.join("projects/speed-edit/evidence/project-plan.md")).unwrap();
    assert!(plan.contains("portflowEvidence: 700"), "{plan}");
    assert!(plan.contains("Project plan.pdf"), "the mirror lists its files: {plan}");
    let project = root.join("projects/speed-edit");
    // An evidence item is one deliverable, named after the item; a second file of its version sits beside it.
    assert_eq!(fs::read(project.join("deliverables/Research.pdf")).unwrap(), bytes_of(3));
    assert_eq!(fs::read(project.join("deliverables/Project plan.pdf")).unwrap(), bytes_of(1));
    assert_eq!(fs::read(project.join("deliverables/Project plan/sources.zip")).unwrap(), bytes_of(2));
    assert_eq!(fs::read(project.join("versions/Project plan@v2.0.pdf")).unwrap(), bytes_of(1));
    assert_eq!(fs::read(project.join("versions/Research@v1.0.pdf")).unwrap(), bytes_of(3));
    let evidence = m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap();
    let plan = evidence.iter().find(|e| e.slug == "project-plan").unwrap();
    assert_eq!(plan.versions.iter().map(|v| v.number).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(
        plan.versions[0].files.iter().map(|f| (f.name.as_str(), f.path.as_deref())).collect::<Vec<_>>(),
        [("plan.pdf", Some("Project plan@v2.0.pdf")), ("sources.zip", Some("Project plan/sources@v2.0.zip"))]
    );
    assert!(plan.versions[0].in_ledger);
    assert_eq!(plan.versions[0].portflow_url.as_deref(), Some("https://portfolio.drieam.app/portfolio/collections/500/evidence/700/versions/7002"));
    assert_eq!(plan.portflow_url.as_deref(), Some("https://portfolio.drieam.app/portfolio/collections/500/evidence/700/versions/latest"));
    assert_eq!(plan.outcomes.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Analysis", "Analysis-U1"]);
}

#[test]
fn feedback_lands_once_the_evidence_is_in_a_thread() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    let first = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(first.new_feedback, 0, "no thread yet, so the comments wait");

    m.edit(|c| {
        threads::create(c, "speed-edit", "Plan").unwrap();
        store::place_evidence(c, "speed-edit", "project-plan", Some("plan")).unwrap();
    });
    let second = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(second.new_feedback, 1, "the coach's comment; the student's own reply doesn't count as new");
    let evidence = m.read(|c| store::evidence_in_thread(c, "speed-edit", "plan", ME)).unwrap();
    assert_eq!((evidence[0].feedback_count, evidence[0].open_feedback), (1, 1), "the student's own reply isn't counted");
    assert_eq!(second.unplaced, 1);

    let all = m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap();
    assert_eq!(all.len(), 2);
    let coach = &all[0];
    assert_eq!(coach.from.as_deref(), Some("Coach Teacher"));
    assert_eq!(coach.from_role.as_deref(), Some("teacher"));
    assert_eq!(coach.given.as_deref(), Some("portflow"));
    assert_eq!(coach.status, "open");
    assert_eq!(coach.evidence.as_deref(), Some("project-plan"));
    assert_eq!(coach.evidence_version, Some(2));
    assert_eq!(coach.deliverable_path, "Project plan@v2.0.pdf", "on the version downloaded from Portflow");
    assert_eq!(coach.body, "Good start, add **risks**.");
    let reply = &all[1];
    assert_eq!(reply.reply_to.as_deref(), Some(coach.id.as_str()));
    assert_eq!(reply.status, "open", "nothing is resolved but by the student");
    assert_eq!(coach.status, "open");

    // Every piece is resolved on its own: the coach's comment doesn't take the reply with it, or the other way round.
    let open_now = |m: &Machine| m.read(|c| store::open_portflow_feedback(c, 10, 0)).unwrap().len();
    assert_eq!(open_now(&m), 2);
    m.edit(|c| feedback::set_status(c, "speed-edit", "plan", &reply.id, "resolved")).unwrap();
    assert_eq!(open_now(&m), 1);
    m.edit(|c| feedback::set_status(c, "speed-edit", "plan", &coach.id, "resolved")).unwrap();
    assert_eq!(open_now(&m), 0, "resolved feedback isn't listed as open");
    m.edit(|c| feedback::set_status(c, "speed-edit", "plan", &reply.id, "open")).unwrap();
    assert_eq!(open_now(&m), 1);
    let pieces = m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap();
    assert_eq!(pieces[0].status, "resolved", "reopening a reply leaves the comment it answers as it was");
    // "Resolve all" and "Reopen all" do the whole card.
    m.edit(|c| feedback::set_status_all(c, "speed-edit", "plan", &coach.id, "resolved")).unwrap();
    assert_eq!(open_now(&m), 0);
    m.edit(|c| feedback::set_status_all(c, "speed-edit", "plan", &coach.id, "open")).unwrap();
    assert_eq!(open_now(&m), 2);

    // Feedback from Portflow can't be edited in Ledger, but can be resolved.
    assert!(m.read(|c| feedback::update(c, "speed-edit", "plan", &coach.id, "changed")).is_err());
    m.edit(|c| feedback::set_status(c, "speed-edit", "plan", &coach.id, "resolved")).unwrap();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let again = m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap();
    assert_eq!(again[0].status, "resolved", "a round doesn't reopen what was resolved");

    // And Epsilon+ counts it on the sub-goal the evidence counts for (not the student's own reply).
    let epsilon = m.read(|c| epsilon::epsilon(c, None, "2026-11-04", ME)).unwrap();
    let analysis_u = epsilon.cells.iter().find(|c| c.outcome == 10 && c.layer == "U").unwrap();
    assert_eq!(analysis_u.feedback_count, 1);
}

#[test]
fn a_round_that_finds_nothing_new_writes_nothing() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    m.edit(|c| {
        threads::create(c, "speed-edit", "Plan").unwrap();
    });
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    m.edit(|c| store::place_evidence(c, "speed-edit", "project-plan", Some("plan")).unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let before = m.files();

    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.files_touched, 0, "{report:?}");
    assert_eq!(m.files(), before);

    // Loading the vault again reads back exactly what was written.
    m.reload();
    assert_eq!(m.files(), before, "the files are canonical");
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.files_touched, 0, "{report:?}");
}

#[test]
fn a_version_published_in_ledger_is_linked_to_its_deliverables_not_copied() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    // A hobby project: its evidence isn't downloaded, so a file is in Ledger only when Ledger published it.
    m.edit(|c| crate::project_facts::save(c, "speed-edit", &crate::project_facts::ProjectMeta { kind: "hobby".into(), goal: "Fun.".into(), ..Default::default() }).unwrap());
    let fake = FakePortflow::new();
    // Ledger published v2.0 of the plan (not of the sources).
    m.edit(|c| crate::deliverables::upsert(c, "speed-edit", "project-plan/plan@v2.0.pdf", 5, "2026-10-02T10:00:00Z", None).unwrap());
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((report.downloaded, report.in_portflow_only), (0, 2), "the sources and the research: named, not copied");
    assert!(fake.downloads.lock().unwrap().is_empty());
    let evidence = m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap();
    let plan = evidence.iter().find(|e| e.slug == "project-plan").unwrap();
    let files: Vec<(&str, Option<&str>)> = plan.versions[0].files.iter().map(|f| (f.name.as_str(), f.path.as_deref())).collect();
    assert_eq!(files, [("plan.pdf", Some("project-plan/plan@v2.0.pdf")), ("sources.zip", None)]);
    assert_eq!(plan.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["project-plan/plan.pdf"]);

    // Publishing the sources' v2.0 later links them on the next round, without asking Portflow for the version again.
    m.edit(|c| crate::deliverables::upsert(c, "speed-edit", "project-plan/sources@v2.0.zip", 5, "2026-10-03T10:00:00Z", None).unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let evidence = m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap();
    let plan = evidence.iter().find(|e| e.slug == "project-plan").unwrap();
    assert!(plan.versions[0].in_ledger);

    // A deliverable with only published versions (no working copy) opens instead of failing.
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "project-plan/plan.pdf")).unwrap();
    assert!(!view.exists);
    assert_eq!(view.versions, ["2.0"]);

    // It is dated by when that version went to Portflow, not by when its file was last touched (synced, checked out).
    let list = m.read(|c| crate::deliverables::list(c, "speed-edit")).unwrap();
    let plan = list.iter().find(|d| d.path == "project-plan/plan.pdf").unwrap();
    let delivered = plan.delivered.as_ref().unwrap();
    assert_eq!((delivered.number, delivered.at.as_str()), (2, "2026-10-02T10:00:00Z"));
    assert_eq!(plan.latest_at.as_deref(), Some("2026-10-02T10:00:00Z"));
}

#[test]
fn a_collection_is_imported_as_a_project_with_a_thread_per_evidence_item() {
    let m = Machine::new();
    m.switch(true);
    let semester = m.edit(|c| {
        semesters::create(
            c,
            &SemesterDraft { name: "Fall 26".into(), start: "2026-09-01".into(), end: "2027-01-31".into(), sprint_weeks: 3, notes: String::new(), sprints: vec![], vacations: vec![] },
        )
        .unwrap()
        .id
    });
    let fake = FakePortflow::new();
    let collection = fake.collections(P).unwrap().remove(0);
    let evidence: Vec<Evidence> = fake.collection_evidence(P, 500).unwrap().into_iter().map(|e| e.evidence).collect();
    let imported = m.edit(|c| store::import_collection(c, &collection, &evidence, &semester, None, true)).unwrap();
    assert_eq!((imported.project.as_str(), imported.project_name.as_str(), imported.evidence, imported.threads), ("speed-edit", "Speed edit", 2, 2));
    let placed = m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap();
    assert!(placed.iter().all(|e| e.thread.is_some()), "every evidence item has its thread");

    // The first sync fills it in, and the feedback lands straight away.
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.unplaced, 0);
    assert_eq!(report.new_feedback, 1);
    let plan = placed.iter().find(|e| e.slug == "project-plan").unwrap();
    assert_eq!(m.read(|c| feedback::list_for_thread(c, "speed-edit", plan.thread.as_deref().unwrap())).unwrap().len(), 2);

    // A collection is imported once.
    assert!(m.edit(|c| store::import_collection(c, &collection, &evidence, &semester, Some("Again"), false)).is_err());
}

#[test]
fn an_outcome_gone_from_portflow_is_kept_as_removed() {
    let m = Machine::new();
    m.switch(true);
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    fake.world.lock().unwrap().goals.pop();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let removed: bool = m.read(|c| c.query_row("SELECT removed FROM outcomes WHERE name = 'Design'", [], |r| r.get(0))).unwrap();
    assert!(removed);
}

// ---- the switch ---------------------------------------------------------------------------------------

#[test]
fn off_is_the_default_and_refuses_a_round_without_asking_anything() {
    let m = Machine::new();
    assert!(!m.settings().enabled);
    let fake = FakePortflow::new();
    assert!(matches!(portflow_sync::run(&fake, &m.ledger, &m.settings()), Err(PortflowError::Off)));
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn switched_off_the_portflow_files_are_left_byte_for_byte() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    m.edit(|c| {
        threads::create(c, "speed-edit", "Plan").unwrap();
    });
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    m.edit(|c| store::place_evidence(c, "speed-edit", "project-plan", Some("plan")).unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let before = m.files();
    assert!(before.keys().any(|k| k.starts_with("shared/portflow/")));

    m.switch(false);
    // The app keeps working: the vault is loaded, edited and written, and the Portflow files stay as they were.
    m.edit(|c| {
        threads::create(c, "speed-edit", "Other").unwrap();
    });
    m.reload();
    let after = m.files();
    for (path, bytes) in &before {
        if path.starts_with("shared/portflow/") || path.contains("/evidence/") || path.contains("/portflow/") || path.contains("/feedback/") || path.contains("/deliverables/") {
            assert_eq!(after.get(path), Some(bytes), "{path} changed while Portflow was off");
        }
    }
    // Nothing Portflow made is in view.
    assert_eq!(m.read(outcome_count), 0);
    assert!(m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap().is_empty());
    // But the links and who gave feedback are kept, in files Ledger always writes.
    let project = fs::read_to_string(m.root().join("projects/speed-edit/project.md")).unwrap();
    assert!(project.contains("portflowCollections"), "{project}");
    let coach = m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap();
    assert_eq!(coach[0].from.as_deref(), Some("Coach Teacher"));

    // Switched on again, it's all back.
    m.switch(true);
    assert_eq!(m.read(outcome_count), 2);
}

#[test]
fn the_worker_runs_only_while_switched_on() {
    let m = Machine::new();
    let emitted = Arc::new(Mutex::new(vec![]));
    let sink = emitted.clone();
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = asked.clone();
    let syncer = crate::portflow::PortflowSyncer::new(
        m.core.clone(),
        Arc::new(move |status: crate::portflow::PortflowStatus| sink.lock().unwrap().push(status.state)),
        Arc::new(|| {}),
        Arc::new(move |_: &Session, _| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakePortflow::new()) as Box<dyn Portflow>)
        }),
        None,
        Duration::from_secs(3600),
    );
    syncer.reconfigure();
    assert!(!syncer.running());
    assert_eq!(syncer.status().state, "off");

    m.switch(true);
    syncer.reconfigure();
    assert!(syncer.running());
    // Without a session (and no way to refresh one) it waits for a sign-in and asks Portflow nothing.
    wait_for(|| emitted.lock().unwrap().contains(&"signed-out".to_string()));
    assert_eq!(asked.load(Ordering::SeqCst), 0);

    syncer.auth.set(Session { token: "a.b.c".into(), csrf: None, portfolio: Some(P), expires_at: None });
    syncer.now();
    wait_for(|| emitted.lock().unwrap().contains(&"idle".to_string()));
    assert_eq!(asked.load(Ordering::SeqCst), 1);

    // The session is kept with the vault's secrets on this machine, so a restart needs no sign-in.
    let token = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjQxMDI0NDQ4MDB9.sig";
    crate::portflow_auth::save_session(m.root(), &Session { token: token.into(), csrf: Some("c".into()), portfolio: None, expires_at: None }).unwrap();
    let restarted = crate::portflow::PortflowSyncer::new(m.core.clone(), Arc::new(|_| {}), Arc::new(|| {}), Arc::new(|_: &Session, _| Err(PortflowError::Off)), None, Duration::from_secs(3600));
    restarted.reconfigure();
    assert_eq!(restarted.auth.session().map(|s| s.token), Some(token.to_string()));
    assert!(fs::read_to_string(crate::settings::secrets_path(m.root())).unwrap().contains("PORTFLOW_SESSION="));

    m.switch(false);
    syncer.reconfigure();
    assert!(!syncer.running());
    assert!(syncer.auth.session().is_none(), "switching off drops the session");
    assert!(crate::portflow_auth::load_session(m.root(), 0).is_none(), "and clears the saved one");
    assert_eq!(syncer.status().state, "off");
}

#[test]
fn another_vault_starts_portflow_again_from_its_own_switch() {
    let m = Machine::new();
    m.switch(true);
    let syncer = crate::portflow::PortflowSyncer::new(
        m.core.clone(),
        Arc::new(|_| {}),
        Arc::new(|| {}),
        Arc::new(|_: &Session, _| Ok(Box::new(FakePortflow::new()) as Box<dyn Portflow>)),
        None,
        Duration::from_secs(3600),
    );
    let token = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjQxMDI0NDQ4MDB9.sig";
    crate::portflow_auth::save_session(m.root(), &Session { token: token.into(), csrf: None, portfolio: Some(P), expires_at: None }).unwrap();
    syncer.reconfigure();
    wait_for(|| syncer.status().state == "idle");
    // A round that started in the old vault, still holding its ledger.
    let stale = CoreLedger { core: m.core.clone(), changed: Arc::new(|| {}), vault: m.root().to_path_buf() };

    // A new vault: Portflow was never switched on in it.
    let other = tempdir().unwrap();
    m.core.lock().unwrap().set_vault(other.path(), false).unwrap();
    syncer.vault_switched();
    assert!(!syncer.running());
    assert_eq!(syncer.status().state, "off", "nothing of the old vault's Portflow shows");
    assert!(syncer.auth.session().is_none(), "the old vault's Canvas session is not this vault's");
    assert!(crate::portflow_auth::load_session(m.root(), 0).is_some(), "but the old vault keeps it for later");
    assert!(stale.write(&mut |_, _| Ok(())).is_err(), "and the old round writes nothing into the new vault");
    assert!(stale.read(&mut |_| Ok(())).is_err());
    assert!(!crate::settings::local_dir(other.path()).join("portflow.json").exists());

    // Back in the old vault, Portflow is on again, signed in with its own session.
    m.core.lock().unwrap().set_vault(m.root(), false).unwrap();
    syncer.vault_switched();
    assert!(syncer.running());
    assert_eq!(syncer.auth.session().map(|s| s.token), Some(token.to_string()));
}

fn wait_for(done: impl Fn() -> bool) {
    for _ in 0..500 {
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out");
}

// ---- who gave feedback ----------------------------------------------------------------------------------

#[test]
fn who_gave_feedback_is_kept_with_portflow_off() {
    let m = Machine::new();
    let id = m.edit(|c| {
        projects::create(c, "Speed edit").unwrap();
        threads::create(c, "speed-edit", "Plan").unwrap();
        crate::deliverables::create(c, "speed-edit", "plan.md", "# Plan").unwrap();
        let from = FeedbackFrom { name: Some("Coach Teacher".into()), portflow_user: Some(100), role: Some("teacher".into()), given: Some("in person".into()) };
        feedback::add_from(c, "speed-edit", "plan", "plan.md", "Add risks", &from).unwrap().id
    });
    m.reload();
    let all = m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap();
    assert_eq!(all[0].id, id);
    assert_eq!(all[0].from.as_deref(), Some("Coach Teacher"));
    assert_eq!(all[0].from_user, Some(100));
    assert_eq!(all[0].given.as_deref(), Some("in person"));
    let text = fs::read_to_string(m.root().join(format!("projects/speed-edit/threads/plan/feedback/{id}.md"))).unwrap();
    let meta = frontmatter::parse(&text).unwrap().meta;
    assert_eq!(frontmatter::MetaExt::text(&meta, "fromRole").as_deref(), Some("teacher"));

    let wrong = FeedbackFrom { given: Some("by pigeon".into()), ..FeedbackFrom::default() };
    assert!(m.read(|c| feedback::update_from(c, "speed-edit", "plan", &id, &wrong)).is_err());
}

// ---- against the real Portflow, by hand -----------------------------------------------------------------

/// One read-only round against the real Portflow into a throwaway vault; prints counts only. Not run by `cargo test`:
///
/// ```text
/// LEDGER_PORTFLOW_TOKEN_FILE=<file with a session token> LEDGER_PORTFLOW_PORTFOLIO=<id> LEDGER_PORTFLOW_COLLECTION=<id> \
///     cargo test real_portflow -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn real_portflow_read_only_round() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let token = fs::read_to_string(var("LEDGER_PORTFLOW_TOKEN_FILE")).unwrap().trim().to_string();
    let portfolio: i64 = var("LEDGER_PORTFLOW_PORTFOLIO").parse().unwrap();
    let collection: i64 = var("LEDGER_PORTFLOW_COLLECTION").parse().unwrap();

    let m = Machine::new();
    config::update_settings(m.root(), |s| {
        s.enabled = true;
        s.portfolio = Some(Portfolio { id: portfolio, user_id: 0, user_name: String::new() });
    })
    .unwrap();
    m.reload();
    let settings = m.settings();
    let semester = m.edit(|c| {
        projects::create(c, "Real").unwrap();
        threads::create(c, "real", "Work").unwrap();
        let s = semesters::create(
            c,
            &SemesterDraft { name: "Now".into(), start: "2026-09-01".into(), end: "2027-01-31".into(), sprint_weeks: 3, notes: String::new(), sprints: vec![], vacations: vec![] },
        )
        .unwrap();
        store::link_collection(c, "real", &s.id, Some(collection)).unwrap();
        s.id
    });
    // Read only: the gate refuses anything but reads.
    let client = HttpPortflow::new(&Credentials { token, csrf: None }, Gate::new(true, &settings.access)).unwrap();
    let first = portflow_sync::run(&client, &m.ledger, &settings).unwrap();
    println!(
        "first round: {} outcomes, {} people, {} collections, {} evidence, {} versions, {} files left in Portflow, {} unplaced, {} notes",
        first.outcomes, first.people, first.collections, first.evidence, first.new_versions, first.in_portflow_only, first.unplaced, first.notes.len()
    );
    // Put every evidence item in the thread, so the comments come in too.
    let slugs: Vec<String> = m.read(|c| store::evidence_in_project(c, "real", ME)).unwrap().into_iter().map(|e| e.slug).collect();
    m.edit(|c| {
        for slug in &slugs {
            store::place_evidence(c, "real", slug, Some("work")).unwrap();
        }
    });
    let second = portflow_sync::run(&client, &m.ledger, &settings).unwrap();
    println!("second round: {} new feedback, {} files touched", second.new_feedback, second.files_touched);
    let third = portflow_sync::run(&client, &m.ledger, &settings).unwrap();
    println!("third round: {} files touched", third.files_touched);
    assert!(!m.root().join("projects/real/deliverables").exists(), "nothing was copied from Portflow");
    assert_eq!(third.files_touched, 0, "an unchanged portfolio writes nothing");
    let feedback = m.read(|c| feedback::list_for_thread(c, "real", "work")).unwrap();
    println!(
        "feedback: {} items, {} with a person, {} replies; collection file: {}",
        feedback.len(),
        feedback.iter().filter(|f| f.from.is_some()).count(),
        feedback.iter().filter(|f| f.reply_to.is_some()).count(),
        m.root().join(format!("projects/real/portflow/{semester}.md")).is_file()
    );
}

// ---- semesters, sections and Epsilon+ ---------------------------------------------------------------------


#[test]
fn a_sub_goals_name_says_its_layer_and_level() {
    assert_eq!(parse_sub_goal("Analysis-U2"), Some((Some("U"), 2)));
    assert_eq!(parse_sub_goal("Manage&Control-H3 "), Some((Some("H"), 3)));
    assert_eq!(parse_sub_goal("Realise-i1"), Some((Some("I"), 1)));
    assert_eq!(parse_sub_goal("PS-3"), Some((None, 3)));
    assert_eq!(parse_sub_goal("Analysis-X1"), None, "no such layer");
    assert_eq!(parse_sub_goal("Reflection"), None);
}

#[test]
fn a_round_links_the_section_to_its_semester_and_fills_the_matrix() {
    let m = Machine::new();
    m.switch(true);
    let semester = m.linked();
    portflow_sync::run(&FakePortflow::new(), &m.ledger, &m.settings()).unwrap();

    // The section is linked to the semester its import fell in, and that is written in the semester's file.
    let sections = m.read(epsilon::sections).unwrap();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].semester.as_deref(), Some(semester.as_str()));
    let file = fs::read_to_string(m.root().join(format!("shared/semesters/{semester}.md"))).unwrap();
    assert!(file.contains("portflowSection: 9"), "{file}");
    assert!(m.root().join("shared/portflow/sections.md").is_file());

    // The current semester's matrix: activities across, layers down, evidence per cell, the highest level with evidence.
    let view = m.read(|c| epsilon::epsilon(c, None, "2026-11-04", ME)).unwrap();
    assert_eq!(view.semester.as_deref(), Some(semester.as_str()));
    assert!(view.scoped);
    assert_eq!(view.section_name.as_deref(), Some("Semester 3 – Fall 26"));
    assert_eq!(view.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Analysis", "Design"]);
    assert_eq!(view.rows.iter().map(|r| r.layer.as_str()).collect::<Vec<_>>(), ["U", "I"], "in HBO-i's order");
    let cell = |layer: &str, outcome: i64| view.cells.iter().find(|c| c.layer == layer && c.outcome == outcome).unwrap();
    assert_eq!((cell("U", 10).evidence_count, cell("U", 10).level), (2, Some(1)), "Portflow counts more than Ledger links");
    assert_eq!(cell("U", 10).levels[0].evidence[0].name, "Project plan");
    assert_eq!((cell("I", 10).evidence_count, cell("I", 10).level), (0, None));
    assert_eq!((cell("U", 20).evidence_count, cell("U", 20).level), (1, Some(1)), "linked in Ledger, not yet counted by Portflow");
    assert_eq!((view.cells_with_evidence, view.cells_total), (2, 3));
    assert_eq!(view.linked_projects, ["speed-edit"], "where a thread started from Epsilon+ goes first");
    assert_eq!(cell("U", 10).levels[0].description, "Analysis-U1 level", "the level's text, to quote");
    assert_eq!(view.max_level, 1);

    // The links survive reading the vault again.
    m.reload();
    let again = m.read(|c| epsilon::epsilon(c, Some(&semester), "2026-11-04", ME)).unwrap();
    assert!(again.scoped);
    assert_eq!(again.cells.iter().map(|c| c.evidence_count).collect::<Vec<_>>(), view.cells.iter().map(|c| c.evidence_count).collect::<Vec<_>>());
}

#[test]
fn a_section_unlinked_by_hand_stays_unlinked() {
    let m = Machine::new();
    m.switch(true);
    let semester = m.linked();
    portflow_sync::run(&FakePortflow::new(), &m.ledger, &m.settings()).unwrap();
    m.edit(|c| epsilon::link_section(c, &semester, None)).unwrap();
    portflow_sync::run(&FakePortflow::new(), &m.ledger, &m.settings()).unwrap();
    assert_eq!(m.read(epsilon::sections).unwrap()[0].semester, None, "a known section isn't guessed again");
    let view = m.read(|c| epsilon::epsilon(c, None, "2026-11-04", ME)).unwrap();
    assert!(!view.scoped, "without a section, the newest goal set is shown");
    assert_eq!(view.columns.len(), 2);
}

#[test]
fn each_semester_shows_its_own_outcomes_and_levels_only_goals_apart() {
    let m = Machine::new();
    m.switch(true);
    let draft = |name: &str, start: &str, end: &str| SemesterDraft { name: name.into(), start: start.into(), end: end.into(), sprint_weeks: 3, ..SemesterDraft::default() };
    let (spring, fall) = m.edit(|c| {
        (semesters::create(c, &draft("Spring 26", "2026-02-09", "2026-07-31")).unwrap().id, semesters::create(c, &draft("Fall 26", "2026-09-01", "2027-01-31")).unwrap().id)
    });
    let import = |id: i64, at: &str| Some(Import { id: Some(id), imported_at: Some(at.into()), template_name: None });
    let goal = |id: i64, name: &str, imp: i64, at: &str| Goal { id, name: name.into(), position: id, last_import: import(imp, at), ..Goal::default() };
    let sub = |id: i64, goal_id: i64, name: &str, count: i64| SubGoal { id, name: name.into(), goal_id, position: id, evidence_count: count, ..SubGoal::default() };
    let goals = vec![
        (goal(1, "Design", 33, "2026-02-01T09:00:00Z"), vec![sub(101, 1, "Design-S1", 3)]),
        (goal(2, "Analysis", 34, "2026-09-21T09:00:00Z"), vec![sub(201, 2, "Analysis-S1", 1), sub(202, 2, "Analysis-S2", 1), sub(203, 2, "Analysis-S3", 0)]),
        (goal(3, "Professional standard", 34, "2026-09-21T09:00:00Z"), vec![sub(301, 3, "PS-1", 2), sub(302, 3, "PS-2", 0)]),
    ];
    let sections = vec![
        Section { id: 1, name: "Semester 2".into(), position: 2, last_import: import(33, "2026-02-01T09:00:00Z") },
        Section { id: 2, name: "Semester 3".into(), position: 1, last_import: import(34, "2026-09-21T09:00:00Z") },
    ];
    m.edit(|c| {
        store::store_outcomes(c, &goals, &[]).unwrap();
        epsilon::store_sections(c, &sections).unwrap();
    });
    let linked: Vec<_> = m.read(epsilon::sections).unwrap().into_iter().map(|s| (s.id, s.semester)).collect();
    assert_eq!(linked, [(2, Some(fall.clone())), (1, Some(spring.clone()))], "an import shortly before a semester starts belongs to it");

    let fall_view = m.read(|c| epsilon::epsilon(c, None, "2026-11-04", ME)).unwrap();
    assert_eq!(fall_view.semester.as_deref(), Some(fall.as_str()));
    assert_eq!(fall_view.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Analysis"]);
    assert_eq!(fall_view.cells[0].level, Some(2));
    assert_eq!(fall_view.max_level, 3);
    assert_eq!(fall_view.skills.len(), 1);
    assert_eq!((fall_view.skills[0].nickname.as_str(), fall_view.skills[0].evidence_count, fall_view.skills[0].level), ("", 2, Some(1)));
    assert_eq!(fall_view.semesters.iter().map(|s| (s.name.as_str(), s.current)).collect::<Vec<_>>(), [("Fall 26", true), ("Spring 26", false)]);

    let spring_view = m.read(|c| epsilon::epsilon(c, Some(&spring), "2026-11-04", ME)).unwrap();
    assert_eq!(spring_view.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Design"]);
    assert!(spring_view.skills.is_empty());
    assert!(m.read(|c| epsilon::epsilon(c, Some("nope"), "2026-11-04", ME)).is_err());
}

#[test]
fn intended_development_marks_the_rows_aimed_for_and_keeps_the_learning_plan() {
    let m = Machine::new();
    m.switch(true);
    let semester = m.linked();
    portflow_sync::run(&FakePortflow::new(), &m.ledger, &m.settings()).unwrap();

    let goals = epsilon::EpsilonGoals {
        layers: BTreeMap::from([("u".into(), 1), ("I".into(), 2)]),
        goals: BTreeMap::new(),
        learning_plan: Some("speed-edit/project-plan".into()),
        study_guide: Some("https://example.org/study-guide".into()),
    };
    m.edit(|c| epsilon::set_goals(c, &semester, &goals)).unwrap();
    let view = m.read(|c| epsilon::epsilon(c, None, "2026-11-04", ME)).unwrap();
    assert_eq!(view.rows.iter().map(|r| (r.layer.as_str(), r.target)).collect::<Vec<_>>(), [("U", Some(1)), ("I", Some(2))], "the letter is read in any case");
    let cell = |layer: &str, outcome: i64| view.cells.iter().find(|c| c.layer == layer && c.outcome == outcome).unwrap();
    assert!(cell("U", 10).reached && cell("U", 20).reached);
    assert!(!cell("I", 10).reached);
    assert_eq!((view.targets_reached, view.targets_total), (2, 3));
    let plan = view.plan.as_ref().unwrap();
    assert_eq!((plan.name.as_str(), plan.version, plan.delivered_at.as_deref()), ("Project plan", Some(2), Some("2026-10-02T10:00:00Z")));

    // Kept in the semester's file, and read back from it.
    let file = fs::read_to_string(m.root().join(format!("shared/semesters/{semester}.md"))).unwrap();
    assert!(file.contains("epsilon:") && file.contains("learningPlan: speed-edit/project-plan"), "{file}");
    m.reload();
    assert_eq!(m.read(|c| epsilon::goals_of(c, &semester)).unwrap().layers, BTreeMap::from([("U".into(), 1), ("I".into(), 2)]));

    // Nonsense is refused, and clearing everything leaves no `epsilon:` behind.
    let bad = |g: epsilon::EpsilonGoals| m.edit(|c| epsilon::set_goals(c, &semester, &g)).is_err();
    assert!(bad(epsilon::EpsilonGoals { layers: BTreeMap::from([("X".into(), 1)]), ..Default::default() }));
    assert!(bad(epsilon::EpsilonGoals { layers: BTreeMap::from([("I".into(), 0)]), ..Default::default() }));
    assert!(bad(epsilon::EpsilonGoals { study_guide: Some("javascript:alert(1)".into()), ..Default::default() }));
    m.edit(|c| epsilon::set_goals(c, &semester, &epsilon::EpsilonGoals::default())).unwrap();
    let file = fs::read_to_string(m.root().join(format!("shared/semesters/{semester}.md"))).unwrap();
    assert!(!file.contains("epsilon:"), "{file}");
}

#[test]
fn a_level_only_goal_is_aimed_for_by_its_nickname() {
    let m = Machine::new();
    m.switch(true);
    let fall = m.edit(|c| {
        semesters::create(c, &SemesterDraft { name: "Fall 26".into(), start: "2026-09-01".into(), end: "2027-01-31".into(), sprint_weeks: 3, ..SemesterDraft::default() }).unwrap().id
    });
    let import = Some(Import { id: Some(34), imported_at: Some("2026-09-21T09:00:00Z".into()), template_name: None });
    let goals = vec![(
        Goal { id: 3, name: "Professional standard".into(), nickname: Some("PS".into()), last_import: import.clone(), ..Goal::default() },
        vec![
            SubGoal { id: 301, name: "PS-1".into(), goal_id: 3, evidence_count: 1, ..SubGoal::default() },
            SubGoal { id: 302, name: "PS-2".into(), goal_id: 3, position: 1, ..SubGoal::default() },
        ],
    )];
    m.edit(|c| {
        store::store_outcomes(c, &goals, &[]).unwrap();
        epsilon::store_sections(c, &[Section { id: 2, name: "Semester 3".into(), position: 1, last_import: import.clone() }]).unwrap();
        epsilon::set_goals(c, &fall, &epsilon::EpsilonGoals { goals: BTreeMap::from([("PS".into(), 2)]), ..Default::default() }).unwrap();
    });
    let view = m.read(|c| epsilon::epsilon(c, None, "2026-11-04", ME)).unwrap();
    let ps = &view.skills[0];
    assert_eq!((ps.key.as_str(), ps.target, ps.reached), ("PS", Some(2), false));
    assert_eq!((view.targets_reached, view.targets_total), (0, 1));
}

// ---- the reading guide in the collection's description -------------------------------------------------------

fn guide_on(m: &Machine, write: bool, kinds: &[&str]) {
    config::update_settings(m.root(), |s| {
        s.access.write = write;
        s.access.writes = kinds.iter().map(|k| k.to_string()).collect();
    })
    .unwrap();
}

/// Makes the linked project a study project: only those have their reading guide kept in step with Portflow.
fn study(m: &Machine) {
    m.edit(|c| {
        crate::project_facts::save(c, "speed-edit", &crate::project_facts::ProjectMeta { kind: "study".into(), goal: "Pass the semester.".into(), ..Default::default() })
            .unwrap()
    });
}

fn readme_of(m: &Machine) -> String {
    readme(m)
}

fn readme(m: &Machine) -> String {
    m.read(|c| projects::get(c, "speed-edit").unwrap().readme)
}

fn portflow_says(fake: &FakePortflow, html: &str) {
    fake.world.lock().unwrap().collections[0].description = Some(html.into());
}

#[test]
fn the_reading_guide_goes_to_portflow_only_when_its_write_is_on() {
    let m = Machine::new();
    m.switch(true);
    let semester = m.linked();
    study(&m);
    let fake = FakePortflow::new();
    // The first round takes Portflow's description into the empty guide.
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    m.edit(|c| projects::save_readme(c, "speed-edit", "# Reading guide\n\nStart with the **project plan**.").unwrap());

    // Off by default: read only, and the write's own switch off.
    assert!(!m.settings().access.allows(config::GUIDE_WRITE));
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_written, 0);
    assert!(fake.descriptions.lock().unwrap().is_empty());
    // Read and write on, but not this write: still nothing.
    guide_on(&m, true, &["upload-versions"]);
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert!(fake.descriptions.lock().unwrap().is_empty());
    // The switch on without read and write: nothing either.
    guide_on(&m, false, &[config::GUIDE_WRITE]);
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert!(fake.descriptions.lock().unwrap().is_empty());
    assert!(readme(&m).contains("project plan"), "Ledger's edit waits, it isn't overwritten");

    // On: Ledger's guide replaces the description. The mirror doesn't keep a copy.
    guide_on(&m, true, &[config::GUIDE_WRITE]);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_written, 1);
    let sent = fake.descriptions.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, 500);
    assert!(sent[0].1.contains("<strong>project plan</strong>"), "{}", sent[0].1);
    let mirrored = fs::read_to_string(m.root().join(format!("projects/speed-edit/portflow/{semester}.md"))).unwrap();
    assert!(!mirrored.contains("project plan") && !mirrored.contains("plan</em>"), "{mirrored}");

    // Unchanged: not sent again.
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(fake.descriptions.lock().unwrap().len(), 1);
    // Changed in Ledger: sent again.
    m.edit(|c| projects::save_readme(c, "speed-edit", "# Reading guide\n\nStart with the research.").unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(fake.descriptions.lock().unwrap().len(), 2);
    // An empty guide never empties the description.
    m.edit(|c| projects::save_readme(c, "speed-edit", "  ").unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(fake.descriptions.lock().unwrap().len(), 2);
}

#[test]
fn an_empty_guide_takes_the_description_and_a_change_in_portflow_comes_in() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    study(&m);
    let fake = FakePortflow::new();

    // First round, nothing remembered, an empty README: it becomes the description, as markdown.
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_read, 1);
    assert_eq!(readme(&m).trim(), "The *plan*.");
    assert!(fs::read_to_string(m.root().join("projects/speed-edit/README.md")).unwrap().contains("The *plan*."));

    // Changed in Portflow, Ledger unchanged: it comes in, write switch or not.
    portflow_says(&fake, "<p>Start with the <strong>research</strong>.</p>");
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_read, 1);
    assert_eq!(readme(&m).trim(), "Start with the **research**.");
    assert!(fake.descriptions.lock().unwrap().is_empty(), "taking it in sends nothing");

    // Nothing changed: nothing happens.
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((report.guides_read, report.guides_written), (0, 0));

    // An emptied description doesn't empty the guide.
    portflow_says(&fake, "<p></p>");
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(readme(&m).trim(), "Start with the **research**.");
}

#[test]
fn an_edit_in_ledger_is_kept_and_sent_only_with_the_write_on() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    study(&m);
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();

    // Edited in Ledger, Portflow unchanged, the write off: kept, nothing sent, no note.
    m.edit(|c| projects::save_readme(c, "speed-edit", "Mine.").unwrap());
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(readme(&m), "Mine.");
    assert!(fake.descriptions.lock().unwrap().is_empty());
    assert!(report.notes.is_empty(), "{:?}", report.notes);

    // Both changed, the write off: Ledger's is kept, and the report says so (once).
    portflow_says(&fake, "<p>Theirs.</p>");
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(readme(&m), "Mine.");
    assert!(report.notes.iter().any(|n| n.contains("both changed")), "{:?}", report.notes);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert!(report.notes.is_empty(), "said once: {:?}", report.notes);

    // The write on: Ledger's text goes to Portflow and wins.
    guide_on(&m, true, &[config::GUIDE_WRITE]);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_written, 1);
    assert!(fake.descriptions.lock().unwrap()[0].1.contains("Mine."));

    // Both changed again, the write on: Ledger's wins and is sent.
    m.edit(|c| projects::save_readme(c, "speed-edit", "Mine, again.").unwrap());
    portflow_says(&fake, "<p>Theirs, again.</p>");
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(readme(&m), "Mine, again.");
    assert!(fake.descriptions.lock().unwrap().last().unwrap().1.contains("Mine, again."));
}

#[test]
fn what_was_seen_survives_a_restart() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    study(&m);
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    m.edit(|c| projects::save_readme(c, "speed-edit", "Mine.").unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert!(crate::settings::local_dir(m.root()).join("portflow-guides.json").exists());

    // A restart: the vault is read again from its files, the memory from this machine's folder for it.
    let m = m.restart();
    m.switch(true);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((report.guides_read, report.guides_written), (0, 0));
    assert_eq!(readme(&m), "Mine.", "the pending edit is still Ledger's, not overwritten");
    // Portflow changes after the restart: both changed, so Ledger's is kept.
    portflow_says(&fake, "<p>Theirs.</p>");
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(readme(&m), "Mine.");
    assert!(report.notes.iter().any(|n| n.contains("both changed")), "{:?}", report.notes);
}

#[test]
fn a_linked_project_that_isnt_a_study_project_is_never_kept_in_step() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    guide_on(&m, true, &[config::GUIDE_WRITE]);
    let fake = FakePortflow::new();
    // A project said to be a work or a hobby one: nothing read, nothing written.
    for kind in ["work", "hobby"] {
        m.edit(|c| crate::project_facts::save(c, "speed-edit", &crate::project_facts::ProjectMeta { kind: kind.into(), goal: "Fun.".into(), ..Default::default() }).unwrap());
        m.edit(|c| projects::save_readme(c, "speed-edit", "Mine.").unwrap());
        portflow_says(&fake, "<p>Theirs.</p>");
        let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
        assert_eq!((report.guides_read, report.guides_written), (0, 0), "{kind:?}");
        assert_eq!(readme(&m), "Mine.");
        assert!(fake.descriptions.lock().unwrap().is_empty());
        assert!(m.read(|c| store::guide_targets(c, "2026-11-16")).unwrap().is_empty());
    }
}

#[test]
fn an_old_mirror_with_the_description_as_its_body_reads_in_and_loses_it() {
    let m = Machine::new();
    m.switch(true);
    let semester = m.linked();
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let file = m.root().join(format!("projects/speed-edit/portflow/{semester}.md"));
    let mut text = fs::read_to_string(&file).unwrap();
    text.push_str("\nThe old copy of the description.\n");
    fs::write(&file, text).unwrap();
    m.reload();
    // Read in without it: nothing in Ledger holds the old copy.
    assert_eq!(m.read(|c| store::project_collections(c, "speed-edit")).unwrap().len(), 1);
    assert!(!readme(&m).contains("old copy"));
    // A file on disk isn't rewritten for nothing; the next change to the mirror leaves the body out.
    fake.world.lock().unwrap().collections[0].name = "Speed edit, renamed".into();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("renamed") && !text.contains("old copy"), "{text}");
}

#[test]
fn the_guide_write_is_a_put_on_the_collection_and_the_gate_holds_it_back() {
    let wire = Arc::new(FakeWire::default());
    let path = format!("/portfolios/{P}/collections/500");
    wire.reply(&path, 200, r#"{"id": 500, "description": "<p>Kept.</p>"}"#);
    let read_only = client(&wire, gate(true, false, &[config::GUIDE_WRITE]));
    assert!(matches!(read_only.set_collection_description(P, 500, "<p>Guide</p>"), Err(PortflowError::NotAllowed(_))));
    let other = client(&wire, gate(true, true, &["upload-versions"]));
    assert!(matches!(other.set_collection_description(P, 500, "<p>Guide</p>"), Err(PortflowError::NotAllowed(_))));
    assert!(wire.sent().is_empty(), "nothing reached Portflow: {:?}", wire.sent());

    let on = client(&wire, gate(true, true, &[config::GUIDE_WRITE]));
    assert_eq!(on.set_collection_description(P, 500, "<p>Guide</p>").unwrap().as_deref(), Some("<p>Kept.</p>"));
    assert_eq!(wire.sent(), vec![("PUT".to_string(), path)]);
    assert_eq!(wire.bodies.lock().unwrap()[0], Some(serde_json::json!({ "description": "<p>Guide</p>" })));
}

#[test]
fn the_guide_goes_to_the_collection_of_the_current_semester() {
    let m = Machine::new();
    m.switch(true);
    m.edit(|c| {
        projects::create(c, "Speed edit").unwrap();
        let mut ids = vec![];
        for (name, start, end) in [("Spring 26", "2026-02-01", "2026-07-15"), ("Fall 26", "2026-09-01", "2027-01-31"), ("Spring 27", "2027-02-01", "2027-07-15")] {
            let s = semesters::create(c, &SemesterDraft { name: name.into(), start: start.into(), end: end.into(), sprint_weeks: 3, notes: String::new(), sprints: vec![], vacations: vec![] }).unwrap();
            ids.push(s.id);
        }
        store::link_collection(c, "speed-edit", &ids[0], Some(400)).unwrap();
        store::link_collection(c, "speed-edit", &ids[1], Some(500)).unwrap();
        store::link_collection(c, "speed-edit", &ids[2], Some(600)).unwrap();
    });
    study(&m);
    let target = |today: &str| m.read(|c| store::guide_targets(c, today)).unwrap().into_iter().map(|g| g.collection).collect::<Vec<_>>();
    assert_eq!(target("2026-11-16"), vec![500], "the semester that covers today");
    assert_eq!(target("2026-03-10"), vec![400]);
    assert_eq!(target("2026-08-10"), vec![400], "between semesters: the one that ended last");
    assert_eq!(target("2025-12-01"), vec![400], "before any: the first to come");
}

#[test]
fn a_machine_that_had_edit_descriptions_on_starts_with_the_guide_write_off() {
    let m = Machine::new();
    let local = crate::settings::local_dir(m.root());
    fs::create_dir_all(&local).unwrap();
    fs::write(local.join("portflow.json"), r#"{"enabled": true, "access": {"write": true, "writes": ["edit-descriptions", "upload-versions"]}}"#).unwrap();
    let settings = m.settings();
    assert!(settings.access.allows("upload-versions"));
    assert!(!settings.access.allows(config::GUIDE_WRITE));
    assert!(!settings.access.writes.contains("edit-descriptions"));
}

#[test]
fn a_saved_launch_address_is_kept_and_a_blank_one_is_the_default() {
    let m = Machine::new();
    let local = crate::settings::local_dir(m.root());
    fs::create_dir_all(&local).unwrap();
    let saved = |url: &str| {
        fs::write(local.join("portflow.json"), serde_json::json!({ "enabled": true, "launchUrl": url }).to_string()).unwrap();
        m.settings().launch_url
    };
    assert_eq!(saved(""), config::DEFAULT_LAUNCH_URL);
    assert_eq!(saved("https://canvas.example.edu/courses/1"), "https://canvas.example.edu/courses/1", "another school's page stays");
}

#[test]
fn only_writes_that_are_listed_can_be_built_and_switched_on() {
    for built in crate::portflow_config::BUILT_WRITES {
        assert!(crate::portflow_config::WRITE_KINDS.iter().any(|(id, _)| *id == built), "{built} is built but not listed");
    }
}

#[test]
fn the_first_round_takes_the_description_and_sets_the_old_guide_aside() {
    // A linked project without facts counts as a study project; its guide says something else than the description.
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    m.edit(|c| projects::save_readme(c, "speed-edit", "Start here.\n\nHi").unwrap());
    portflow_says(&fake, "<h1>Introduction</h1><p>The real guide.</p>");
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_read, 1);
    assert!(readme(&m).contains("The real guide."), "{}", readme(&m));
    assert!(report.notes.iter().any(|n| n.contains("conflicts folder")), "{:?}", report.notes);
    let kept = walkdir_files(&crate::settings::local_dir(m.root()).join("conflicts"));
    assert!(kept.iter().any(|(path, text)| path.ends_with("portflow/speed-edit-README.md") && text.contains("Hi")), "{kept:?}");
    assert!(fake.descriptions.lock().unwrap().is_empty(), "nothing is written on the way in");
}

/// Every file under `dir`, as (path, text).
fn walkdir_files(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = vec![];
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walkdir_files(&path));
        } else {
            out.push((path.to_string_lossy().replace('\\', "/"), std::fs::read_to_string(&path).unwrap_or_default()));
        }
    }
    out
}

#[test]
fn mentions_come_in_with_their_ids_stay_agreed_and_go_back_as_spans() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    m.edit(|c| {
        threads::create(c, "speed-edit", "Plan").unwrap();
    });
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap(); // the evidence items come in
    m.edit(|c| store::place_evidence(c, "speed-edit", "project-plan", Some("plan")).unwrap());
    let span = r#"<span data-type="mention" data-mention-type="evidence" data-id="evidence-700" data-label="Project plan">@Project plan</span>"#;
    portflow_says(&fake, &format!("<p>Start with {span}.</p>"));
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_read, 1);
    assert_eq!(readme(&m).trim(), "Start with [@Project plan](portflow:evidence-700).");
    // It renders as a chip in Ledger.
    let html = crate::markdown::render(&readme(&m));
    assert!(html.contains(r#"<a class="mention" data-mention="evidence-700" data-mention-type="evidence">@Project plan</a>"#), "{html}");

    // Pulled and unchanged: nothing to do, on either side.
    guide_on(&m, true, &[config::GUIDE_WRITE]);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((report.guides_read, report.guides_written), (0, 0));
    assert!(fake.descriptions.lock().unwrap().is_empty());

    // Edited in Ledger, with a [[link]] to the thread the evidence item is in: both go to Portflow as mentions.
    m.edit(|c| projects::save_readme(c, "speed-edit", "Start with [@Project plan](portflow:evidence-700), see [[Plan]].").map(|_| ()).unwrap());
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.guides_written, 1);
    let sent = fake.descriptions.lock().unwrap().last().unwrap().1.clone();
    assert_eq!(sent.matches(span).count(), 2, "{sent}");
    // And what was sent isn't seen as a change on the next round.
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((report.guides_read, report.guides_written), (0, 0));
}

#[test]
fn a_guide_taken_in_with_bold_mentions_gets_their_ids() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    let span = r#"<span data-type="mention" data-mention-type="evidence" data-id="evidence-700" data-label="Project plan">@Project plan</span>"#;
    portflow_says(&fake, &format!("<p>See {span}.</p>"));
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    // As an older Ledger took it in: the mention as a bold label, and that remembered as agreed.
    m.edit(|c| projects::save_readme(c, "speed-edit", "See **@Project plan**.").map(|_| ()).unwrap());
    let mut memory = config::read_guides(m.root());
    for seen in memory.guides.values_mut() {
        seen.readme = crate::provenance::sha256_hex("See **@Project plan**.".as_bytes());
    }
    config::write_guides(m.root(), &memory).unwrap();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(readme(&m).trim(), "See [@Project plan](portflow:evidence-700).");
}

// ---- evidence comes in as deliverables ----------------------------------------------------------------

/// The paths that differ between two looks at the vault, with what the second one says.
fn changed(before: &BTreeMap<String, Vec<u8>>, after: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    keys.into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .map(|k| format!("{k}: {}", after.get(k).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_else(|| "(gone)".into())))
        .collect()
}

fn downloads(fake: &FakePortflow) -> Vec<String> {
    fake.downloads.lock().unwrap().clone()
}

#[test]
fn each_file_is_downloaded_once_and_its_versions_are_published() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    study(&m);
    let fake = FakePortflow::new();
    // The research is in a thread before it comes in: its versions are filed there.
    m.edit(|c| {
        threads::create(c, "speed-edit", "Research").unwrap();
        store::store_evidence(c, &store::collection_links(c).unwrap()[0], &fake.world.lock().unwrap().evidence[1], &[]).unwrap();
        store::place_evidence(c, "speed-edit", "research", Some("research")).unwrap();
    });
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(downloads(&fake).len(), 3);

    // Each version is a published version, recorded as coming from Portflow.
    let record: (String, String, Option<String>) = m.read(|c| {
        c.query_row(
            "SELECT note, author, thread_slug FROM version_records WHERE project_slug = 'speed-edit' AND deliverable_path = 'Research.pdf' AND version = '1.0'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    });
    assert_eq!(record.0, "From Portflow, version 1");
    assert_eq!(record.1, "human");
    assert_eq!(record.2.as_deref(), Some("research"), "filed in the evidence item's thread");
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "Project plan.pdf")).unwrap();
    assert!(view.exists);
    assert_eq!(view.versions, ["2.0"]);

    // The second round downloads nothing.
    let before = m.files();
    let again = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((again.downloaded, again.new_versions), (0, 0));
    assert_eq!(downloads(&fake).len(), 3);
    assert_eq!(changed(&before, &m.files()), Vec::<String>::new());

    // A new version in Portflow: one more download, a new published version, and the live file is that version.
    fake.world.lock().unwrap().versions.get_mut(&800).unwrap().insert(0, version(8002, 800, 2, "file", vec![file(4, "research.pdf")]));
    let third = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((third.new_versions, third.downloaded), (1, 1));
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "Research.pdf")).unwrap();
    assert_eq!(view.versions, ["1.0", "2.0"]);
    let project = m.root().join("projects/speed-edit");
    assert_eq!(fs::read(project.join("deliverables/Research.pdf")).unwrap(), bytes_of(4));
    assert_eq!(fs::read(project.join("versions/Research@v1.0.pdf")).unwrap(), bytes_of(3), "the old version stays");

    // Another machine reading the vault knows the files are there: nothing to download.
    m.reload();
    let fourth = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((fourth.downloaded, fourth.files_touched), (0, 0), "{fourth:?}");
}

#[test]
fn a_live_file_changed_in_ledger_is_kept_and_a_new_name_never_clobbers() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    // A file of the same name that isn't the evidence item's: the download goes into the item's folder instead.
    let project = m.root().join("projects/speed-edit");
    fs::create_dir_all(project.join("deliverables")).unwrap();
    fs::write(project.join("deliverables/Research.pdf"), b"my own research").unwrap();
    m.edit(|c| crate::deliverables::upsert(c, "speed-edit", "Research.pdf", 15, "2026-10-01T10:00:00Z", None).unwrap());
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(fs::read(project.join("deliverables/Research.pdf")).unwrap(), b"my own research");
    assert_eq!(fs::read(project.join("deliverables/Research-2.pdf")).unwrap(), bytes_of(3));
    // And it isn't renamed onto the other one later.
    assert_eq!(portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap().renamed, 0);

    // The plan, changed in Ledger after it came in: a new Portflow version doesn't overwrite it.
    fs::write(project.join("deliverables/Project plan.pdf"), b"edited here").unwrap();
    fake.world.lock().unwrap().versions.get_mut(&700).unwrap().insert(0, version(7003, 700, 3, "file", vec![file(5, "plan.pdf")]));
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.downloaded, 1);
    assert_eq!(fs::read(project.join("deliverables/Project plan.pdf")).unwrap(), b"edited here");
    assert_eq!(fs::read(project.join("versions/Project plan@v3.0.pdf")).unwrap(), bytes_of(5));
    assert!(report.notes.iter().any(|n| n.contains("Project plan.pdf") && n.contains("stays")), "{:?}", report.notes);
}

#[test]
fn a_failed_download_is_a_note_and_is_tried_again() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    fake.broken.lock().unwrap().insert(3);
    let first = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((first.downloaded, first.in_portflow_only), (2, 1));
    assert!(first.notes.iter().any(|n| n.contains("research.pdf") && n.contains("tries again")), "{:?}", first.notes);
    assert!(!m.root().join("projects/speed-edit/deliverables/Research.pdf").exists());

    fake.broken.lock().unwrap().clear();
    let second = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((second.downloaded, second.in_portflow_only), (1, 0));
    assert!(second.notes.is_empty(), "{:?}", second.notes);
    assert_eq!(fs::read(m.root().join("projects/speed-edit/deliverables/Research.pdf")).unwrap(), bytes_of(3));
    assert_eq!(downloads(&fake).iter().filter(|u| u.contains("/files/3?")).count(), 2);
}

#[test]
fn the_session_never_goes_to_the_files_address() {
    let wire = Arc::new(FakeWire::default());
    wire.reply(&format!("/portfolios/{P}/evidence-version-files/42/preview-url"), 200, r#"{"url": "https://bucket.s3.example/plan.pdf?X-Amz-Signature=x"}"#);
    let fetched: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = fetched.clone();
    let c = client(&wire, gate(true, false, &[])).with_fetch(Arc::new(move |url: &str| {
        seen.lock().unwrap().push(url.to_string());
        Ok(b"bytes".to_vec())
    }));
    let url = c.file_url(P, 42).unwrap();
    assert_eq!(c.download(&url).unwrap(), b"bytes");
    // The address is asked for through the gate (read-only is enough), the file fetched apart, given only the address.
    assert_eq!(wire.sent(), [("GET".to_string(), format!("/portfolios/{P}/evidence-version-files/42/preview-url"))]);
    assert_eq!(*fetched.lock().unwrap(), ["https://bucket.s3.example/plan.pdf?X-Amz-Signature=x"]);
    // A bare string answer works too; an address that isn't https is refused.
    wire.reply(&format!("/portfolios/{P}/evidence-version-files/43/preview-url"), 200, r#""https://bucket.s3.example/b.pdf""#);
    assert_eq!(c.file_url(P, 43).unwrap(), "https://bucket.s3.example/b.pdf");
    wire.reply(&format!("/portfolios/{P}/evidence-version-files/44/preview-url"), 200, r#"{"url": "http://bucket.s3.example/c.pdf"}"#);
    assert!(matches!(c.file_url(P, 44), Err(PortflowError::Changed(_))));
    // And the real fetch refuses plain http before sending anything.
    assert!(plain_fetch()("http://example.com/x").is_err());
}

#[test]
fn a_mention_of_an_evidence_item_names_its_deliverable() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let mentions = m.read(|c| store::evidence_mentions(c, "speed-edit")).unwrap();
    let plan = mentions.iter().find(|e| e.portflow_id == 700).unwrap();
    assert_eq!(plan.deliverable.as_deref(), Some("Project plan.pdf"));
    let research = mentions.iter().find(|e| e.portflow_id == 800).unwrap();
    assert_eq!(research.deliverable.as_deref(), Some("Research.pdf"));
}

#[test]
fn the_deliverable_has_the_evidence_items_name_not_the_files() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    {
        let mut world = fake.world.lock().unwrap();
        world.evidence[1].name = "Project plan DAE".into();
        world.versions.insert(800, vec![version(8001, 800, 1, "file", vec![file(3, "Project plan.pdf")])]);
    }
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let project = m.root().join("projects/speed-edit");
    assert_eq!(fs::read(project.join("deliverables/Project plan DAE.pdf")).unwrap(), bytes_of(3));
    assert_eq!(fs::read(project.join("versions/Project plan DAE@v1.0.pdf")).unwrap(), bytes_of(3));
    assert_eq!(fs::read(project.join("deliverables/Project plan.pdf")).unwrap(), bytes_of(1), "the other item's, by its own name");

    // One deliverable per item, shown by the item's name; a second file of a version is folded into it.
    let list = m.read(|c| crate::deliverables::list(c, "speed-edit")).unwrap();
    let rows: Vec<(&str, &str, Option<&str>)> = list.iter().map(|d| (d.path.as_str(), d.name.as_str(), d.evidence.as_deref())).collect();
    assert_eq!(rows, [("Project plan.pdf", "Project plan", Some("Project plan")), ("Project plan DAE.pdf", "Project plan DAE", Some("Project plan DAE"))]);
    assert_eq!(list[0].attachments, ["Project plan/sources.zip"]);
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "Project plan DAE.pdf")).unwrap();
    assert_eq!((view.name.as_str(), view.versions.as_slice()), ("Project plan DAE", ["1.0".to_string()].as_slice()));
    let plan = m.read(|c| crate::deliverables::get(c, "speed-edit", "Project plan.pdf")).unwrap();
    assert_eq!(plan.attachments, ["Project plan/sources.zip"]);
    // The attachment still opens on its own page.
    assert!(m.read(|c| crate::deliverables::get(c, "speed-edit", "Project plan/sources.zip")).unwrap().exists);
}

#[test]
fn a_rename_in_portflow_renames_the_deliverable_with_its_versions() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    m.edit(|c| {
        threads::create(c, "speed-edit", "Plan").unwrap();
    });
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    m.edit(|c| store::place_evidence(c, "speed-edit", "project-plan", Some("plan")).unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap()[0].deliverable_path, "Project plan@v2.0.pdf");

    {
        let mut world = fake.world.lock().unwrap();
        world.evidence[0].name = "Plan of approach".into();
        world.evidence[1].name = "Research report".into();
    }
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((report.renamed, report.downloaded), (3, 0), "both deliverables and the plan's attachment: {report:?}");
    let project = m.root().join("projects/speed-edit");
    assert_eq!(fs::read(project.join("deliverables/Research report.pdf")).unwrap(), bytes_of(3));
    assert_eq!(fs::read(project.join("versions/Research report@v1.0.pdf")).unwrap(), bytes_of(3));
    assert_eq!(fs::read(project.join("deliverables/Plan of approach.pdf")).unwrap(), bytes_of(1));
    assert_eq!(fs::read(project.join("deliverables/Plan of approach/sources.zip")).unwrap(), bytes_of(2));
    assert_eq!(fs::read(project.join("versions/Plan of approach/sources@v2.0.zip")).unwrap(), bytes_of(2));
    for gone in ["deliverables/Research.pdf", "versions/Research@v1.0.pdf", "deliverables/Project plan.pdf", "deliverables/Project plan", "versions/Project plan"] {
        assert!(!project.join(gone).exists(), "{gone} is still there");
    }
    // What pointed at it points at the new name: its record, the feedback on it, the version in Portflow, the mention.
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "Research report.pdf")).unwrap();
    assert_eq!((view.name.as_str(), view.versions.as_slice()), ("Research report", ["1.0".to_string()].as_slice()));
    let note: String = m.read(|c| c.query_row("SELECT note FROM version_records WHERE deliverable_path = 'Research report.pdf'", [], |r| r.get(0)).unwrap());
    assert_eq!(note, "From Portflow, version 1");
    assert_eq!(m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap()[0].deliverable_path, "Plan of approach@v2.0.pdf");
    let evidence = m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap();
    let plan = evidence.iter().find(|e| e.slug == "project-plan").unwrap();
    assert_eq!(plan.versions[0].files.iter().map(|f| f.path.as_deref()).collect::<Vec<_>>(), [Some("Plan of approach@v2.0.pdf"), Some("Plan of approach/sources@v2.0.zip")]);
    let mentions = m.read(|c| store::evidence_mentions(c, "speed-edit")).unwrap();
    assert_eq!(mentions.iter().find(|e| e.portflow_id == 800).unwrap().deliverable.as_deref(), Some("Research report.pdf"));

    // Read back from the files, nothing moves again.
    m.reload();
    let again = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!((again.renamed, again.downloaded, again.files_touched), (0, 0, 0), "{again:?}");
}

#[test]
fn a_deliverable_made_in_ledger_keeps_its_file_name_but_shows_the_items() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    // Ledger handed in v1.0 of its own "research-notes.pdf" as the research item.
    let project = m.root().join("projects/speed-edit");
    fs::create_dir_all(project.join("versions")).unwrap();
    fs::write(project.join("versions/research-notes@v1.0.pdf"), bytes_of(3)).unwrap();
    m.edit(|c| crate::deliverables::upsert(c, "speed-edit", "research-notes@v1.0.pdf", 5, "2026-10-01T10:00:00Z", None).unwrap());
    let fake = FakePortflow::new();
    fake.world.lock().unwrap().versions.insert(800, vec![version(8001, 800, 1, "file", vec![file(3, "research-notes@v1.0.pdf")])]);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert!(!downloads(&fake).iter().any(|u| u.contains("/files/3?")), "linked, not downloaded");
    assert_eq!(report.renamed, 0);
    let list = m.read(|c| crate::deliverables::list(c, "speed-edit")).unwrap();
    let notes = list.iter().find(|d| d.path == "research-notes.pdf").unwrap();
    assert_eq!((notes.name.as_str(), notes.evidence.as_deref()), ("Research", Some("Research")));
}

#[test]
fn a_new_version_under_another_file_name_is_the_next_version_of_the_same_deliverable() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    fake.world.lock().unwrap().versions.get_mut(&800).unwrap().insert(0, version(8002, 800, 2, "file", vec![file(6, "Research final.pdf")]));
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "Research.pdf")).unwrap();
    assert_eq!(view.versions, ["1.0", "2.0"]);
    assert!(view.attachments.is_empty());
    let project = m.root().join("projects/speed-edit");
    assert_eq!(fs::read(project.join("deliverables/Research.pdf")).unwrap(), bytes_of(6));
    assert!(!project.join("deliverables/Research").exists());
}

#[test]
fn an_older_layout_in_a_folder_of_the_items_comes_to_the_top_under_the_items_name() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    // What an older import left: the item's file in a folder of its own, its versions listed as `<file>: <version>`.
    let project = m.root().join("projects/speed-edit");
    fs::create_dir_all(project.join("versions/research")).unwrap();
    fs::create_dir_all(project.join("evidence")).unwrap();
    fs::write(project.join("versions/research/old-research@v1.0.pdf"), bytes_of(3)).unwrap();
    fs::write(
        project.join("versions/portflow-8001-old-research-pdf.md"),
        "---\nid: portflow-8001-old-research-pdf\ndeliverable: research/old-research.pdf\nversion: '1.0'\nsha256: x\ncreatedAt: 2026-10-01T10:00:00Z\n---\n\nVersion 1 in Portflow\n",
    )
    .unwrap();
    fs::write(
        project.join("evidence/research.md"),
        "---\nname: Research\nfiles:\n- research/old-research.pdf\nportflowEvidence: 800\ntype: file\ncreatedAt: 2026-09-02T10:00:00Z\nupdatedAt: 2026-09-02T10:00:00Z\nversions:\n- number: 1\n  portflowVersion: 8001\n  type: file\n  createdAt: 2026-10-01T10:00:00Z\n  files:\n    research/old-research.pdf: research/old-research@v1.0.pdf\n---\n",
    )
    .unwrap();
    m.reload();
    let fake = FakePortflow::new();
    fake.world.lock().unwrap().versions.get_mut(&800).unwrap().insert(0, version(8002, 800, 2, "file", vec![file(6, "Research final.pdf")]));
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert!(!downloads(&fake).iter().any(|u| u.contains("/files/3?")), "version 1 is in Ledger already");
    assert_eq!(report.renamed, 1, "{report:?}");
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "Research.pdf")).unwrap();
    assert_eq!(view.versions, ["1.0", "2.0"]);
    assert_eq!(fs::read(project.join("versions/Research@v1.0.pdf")).unwrap(), bytes_of(3));
    assert_eq!(fs::read(project.join("deliverables/Research.pdf")).unwrap(), bytes_of(6));
    assert!(!project.join("versions/research").exists() && !project.join("deliverables/research").exists());
    let mirror = fs::read_to_string(project.join("evidence/research.md")).unwrap();
    assert!(mirror.contains("deliverable: Research@v1.0.pdf") && mirror.contains("- Research.pdf"), "{mirror}");
    m.reload();
    assert_eq!(portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap().files_touched, 0);
}

#[test]
fn of_an_items_files_the_one_named_like_it_is_the_deliverable() {
    assert_eq!(crate::deliverables::main_of("Workflow and Bottleneck Report", &["w/02-interview.pdf", "w/02-workflow-and-bottleneck-report.pdf"]), 1);
    assert_eq!(crate::deliverables::main_of("Project plan DAE", &["Project plan.pdf"]), 0);
    assert_eq!(crate::deliverables::main_of("Demo", &["a.pdf", "b.pdf"]), 0, "none matches: the first");
    // A version with two files: the one named like the item is the deliverable, the other its attachment.
    let m = Machine::new();
    m.switch(true);
    m.linked();
    let fake = FakePortflow::new();
    fake.world.lock().unwrap().versions.insert(800, vec![version(8001, 800, 1, "file", vec![file(3, "02-interview.pdf"), file(4, "02-research.pdf")])]);
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let view = m.read(|c| crate::deliverables::get(c, "speed-edit", "Research.pdf")).unwrap();
    assert_eq!(fs::read(m.root().join("projects/speed-edit/deliverables/Research.pdf")).unwrap(), bytes_of(4));
    assert_eq!(view.attachments, ["Research/02-interview.pdf"]);
}

// ---- an evidence item's description ---------------------------------------------------------------------------

const TEACHERS_HTML: &str = "<p>Evidence description</p><p>What? A written plan with the main question.</p><p>Goal? To give my expert a clear picture.</p><p>Why it contributes? It shows that I analyse the problem.</p>";

fn portflow_describes(fake: &FakePortflow, evidence: i64, html: &str) {
    fake.world.lock().unwrap().evidence.iter_mut().find(|e| e.id == evidence).unwrap().description = Some(html.into());
}

fn described(m: &Machine, evidence: i64) -> String {
    m.read(|c| c.query_row("SELECT description FROM evidence WHERE portflow_id = ?1", [evidence], |r| r.get::<_, String>(0))).unwrap()
}

fn describe(m: &Machine, evidence: i64, text: &str) {
    let slug = m.read(|c| store::evidence_slug_for(c, "speed-edit", evidence)).unwrap().unwrap();
    m.edit(|c| store::set_evidence_description(c, "speed-edit", &slug, text).unwrap());
}

#[test]
fn a_description_in_the_teachers_shape_comes_in_as_its_three_parts() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    study(&m);
    let fake = FakePortflow::new();
    portflow_describes(&fake, 700, TEACHERS_HTML);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.descriptions_read, 1);
    let text = described(&m, 700);
    assert_eq!(
        text,
        "**Evidence description**\n\n**What?** A written plan with the main question.\n\n**Goal?** To give my expert a clear picture.\n\n**Why it contributes?** It shows that I analyse the problem."
    );
    // The body of the item's file, so it is readable outside Ledger and survives a reload.
    let slug = m.read(|c| store::evidence_slug_for(c, "speed-edit", 700)).unwrap().unwrap();
    let file = fs::read_to_string(m.root().join(format!("projects/speed-edit/evidence/{slug}.md"))).unwrap();
    assert!(file.contains("**What?** A written plan"), "{file}");
    let parts = crate::portflow_evidence_text::parse(&text).unwrap();
    assert_eq!(parts.goal, "To give my expert a clear picture.");

    // Unchanged: nothing more. Changed in Portflow only: it comes in.
    assert_eq!(portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap().descriptions_read, 0);
    portflow_describes(&fake, 700, "<p>What? A plan, rewritten.</p>");
    assert_eq!(portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap().descriptions_read, 1);
    assert_eq!(described(&m, 700), "**Evidence description**\n\n**What?** A plan, rewritten.");
    assert!(fake.evidence_descriptions.lock().unwrap().is_empty(), "taking it in sends nothing");

    // Free text stays as it is.
    portflow_describes(&fake, 800, "<p>Just what I found.</p>");
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(described(&m, 800), "Just what I found.");
}

#[test]
fn an_edit_in_ledger_goes_to_the_evidence_item_only_with_its_write_on() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    study(&m);
    let fake = FakePortflow::new();
    portflow_describes(&fake, 700, TEACHERS_HTML);
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    let mine = "**What?** My plan.\n\n**Goal?** Clarity.\n\n**Why it contributes?** Analysing, level 2.";
    describe(&m, 700, mine);

    // Off, read and write on with another write, or the switch on without read and write: nothing goes out.
    for (write, kinds) in [(false, vec![]), (true, vec![config::GUIDE_WRITE]), (false, vec![config::EVIDENCE_WRITE])] {
        guide_on(&m, write, &kinds);
        let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
        assert_eq!(report.descriptions_written, 0);
        assert!(report.notes.is_empty(), "{:?}", report.notes);
    }
    assert!(fake.evidence_descriptions.lock().unwrap().is_empty());
    assert_eq!(described(&m, 700), mine, "Ledger's edit waits, it isn't overwritten");

    // On: sent in the shape teachers like.
    guide_on(&m, true, &[config::EVIDENCE_WRITE]);
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(report.descriptions_written, 1);
    let sent = fake.evidence_descriptions.lock().unwrap().clone();
    assert_eq!(sent[0].0, 700);
    assert!(sent[0].1.starts_with("<p><strong>Evidence description</strong></p>"), "{}", sent[0].1);
    assert!(sent[0].1.contains("<p><strong>Goal?</strong> Clarity.</p>"), "{}", sent[0].1);
    // Not sent again while nothing changes; an empty text never empties Portflow's.
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    describe(&m, 700, " ");
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(fake.evidence_descriptions.lock().unwrap().len(), 1);
}

#[test]
fn when_both_sides_changed_the_description_ledger_keeps_its_own() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    study(&m);
    let fake = FakePortflow::new();
    portflow_describes(&fake, 700, TEACHERS_HTML);
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();

    describe(&m, 700, "**What?** Mine.");
    portflow_describes(&fake, 700, "<p>What? Theirs.</p>");
    let report = portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(described(&m, 700), "**What?** Mine.");
    assert!(report.notes.iter().any(|n| n.contains("changed in Ledger and in Portflow")), "{:?}", report.notes);
    assert!(portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap().notes.is_empty(), "said once");

    // The write on: Ledger's goes to Portflow.
    guide_on(&m, true, &[config::EVIDENCE_WRITE]);
    assert_eq!(portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap().descriptions_written, 1);
    assert!(fake.evidence_descriptions.lock().unwrap()[0].1.contains("Mine."));
}

#[test]
fn a_hobby_project_keeps_its_evidence_descriptions_to_itself() {
    let m = Machine::new();
    m.switch(true);
    m.linked();
    m.edit(|c| crate::project_facts::save(c, "speed-edit", &crate::project_facts::ProjectMeta { kind: "hobby".into(), goal: "Fun.".into(), ..Default::default() }).unwrap());
    let fake = FakePortflow::new();
    portflow_describes(&fake, 700, TEACHERS_HTML);
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
    assert_eq!(described(&m, 700), "");
}

#[test]
fn the_evidence_description_write_is_a_put_on_the_item_and_the_gate_holds_it_back() {
    let wire = Arc::new(FakeWire::default());
    let path = format!("/portfolios/{P}/evidence/700");
    wire.reply(&path, 200, r#"{"id": 700, "description": "<p>Kept.</p>"}"#);
    let guide_only = client(&wire, gate(true, true, &[config::GUIDE_WRITE]));
    assert!(matches!(guide_only.set_evidence_description(P, 700, "<p>Mine</p>"), Err(PortflowError::NotAllowed(_))));
    let read_only = client(&wire, gate(true, false, &[config::EVIDENCE_WRITE]));
    assert!(matches!(read_only.set_evidence_description(P, 700, "<p>Mine</p>"), Err(PortflowError::NotAllowed(_))));
    assert!(wire.sent().is_empty(), "nothing reached Portflow: {:?}", wire.sent());

    let on = client(&wire, gate(true, true, &[config::EVIDENCE_WRITE]));
    assert_eq!(on.set_evidence_description(P, 700, "<p>Mine</p>").unwrap().as_deref(), Some("<p>Kept.</p>"));
    assert_eq!(wire.sent(), vec![("PUT".to_string(), path)]);
    assert_eq!(wire.bodies.lock().unwrap()[0], Some(serde_json::json!({ "description": "<p>Mine</p>" })));
    // Only the item itself: a version upload under it is still its own kind.
    let request = crate::portflow_api::Request { method: "POST", path: format!("/portfolios/{P}/evidence/700/versions"), query: vec![], body: None };
    assert_eq!(crate::portflow_api::write_kind(&request), Some("upload-versions"));
}
