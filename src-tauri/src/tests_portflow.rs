//! Portflow against a fake one: no network, no real session. The fake holds a small portfolio (outcomes, people, a
//! semester's collection with two evidence items, versions with files, comments with a reply) and counts every call,
//! so the tests can say exactly what was asked, and that nothing was asked at all while the switch is off.

use std::collections::BTreeMap;
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
use crate::portflow_store as store;
use crate::portflow_sync::{self, CoreLedger};
use crate::state::Core;
use crate::{feedback, frontmatter, projects, semesters, threads};

const P: i64 = 22497;
const ME: i64 = 7;

// ---- the gate and the client, with a fake wire --------------------------------------------------------

#[derive(Default)]
struct FakeWire {
    sent: Mutex<Vec<(String, String)>>,
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
    let all = client(&wire, gate(true, true, &["upload-versions", "create-evidence", "edit-descriptions", "link-outcomes", "reply-feedback", "ask-feedback", "confirm-feedback"]));
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
        FakePortflow { world: Mutex::new(world), calls: AtomicUsize::new(0) }
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
        let config = tempdir().unwrap();
        let core = Arc::new(Mutex::new(Core::open_at(dir.path().to_path_buf(), config.path().to_path_buf(), false).unwrap()));
        let ledger = CoreLedger { core: core.clone(), changed: Arc::new(|| {}) };
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
                if rel.starts_with(".ledger") {
                    continue;
                }
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
    assert_eq!(report.in_portflow_only, 3, "uploaded in Portflow: named, not copied");

    let outcomes = m.read(|c| store::outcomes(c, ME)).unwrap();
    assert_eq!(outcomes.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Analysis", "Design"]);
    assert_eq!(outcomes[0].sub_outcomes.len(), 2);
    assert_eq!(outcomes[0].sub_outcomes[0].evidence[0].name, "Project plan");
    let people = m.read(store::people).unwrap();
    assert_eq!(people.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Coach Teacher", "Peer Student"], "teachers first");

    let collections = m.read(|c| store::project_collections(c, "speed-edit")).unwrap();
    assert_eq!(collections[0].name, "Speed edit");
    assert_eq!(collections[0].section, "Semester 3 – Fall 26");
    assert!(collections[0].description_html.contains("<em>plan</em>"));
    assert_eq!(collections[0].outcomes[0].name, "Analysis");

    // The files: the mirror, the collection and the evidence items; no copies of Portflow's files.
    let root = m.root();
    assert!(root.join("portflow/outcomes/10-analysis.md").is_file());
    assert!(root.join("portflow/people.md").is_file());
    assert!(root.join(format!("projects/speed-edit/portflow/{semester}.md")).is_file());
    let plan = fs::read_to_string(root.join("projects/speed-edit/evidence/project-plan.md")).unwrap();
    assert!(plan.contains("portflowEvidence: 700"), "{plan}");
    assert!(!root.join("projects/speed-edit/deliverables").exists());
    let evidence = m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap();
    let plan = evidence.iter().find(|e| e.slug == "project-plan").unwrap();
    assert_eq!(plan.versions.iter().map(|v| v.number).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(plan.versions[0].files.iter().map(|f| (f.name.as_str(), f.path.is_none())).collect::<Vec<_>>(), [("plan.pdf", true), ("sources.zip", true)]);
    assert!(!plan.versions[0].in_ledger);
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
    assert_eq!(coach.deliverable_path, "", "on a version that is only in Portflow: no deliverable");
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

    // And the outcome knows who gave feedback on it.
    let outcomes = m.read(|c| store::outcomes(c, ME)).unwrap();
    assert_eq!(outcomes[0].feedback_count, 1);
    assert_eq!(outcomes[0].feedback_from, ["Coach Teacher"]);
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
    let fake = FakePortflow::new();
    // Ledger published v2.0 of the plan (not of the sources).
    m.edit(|c| crate::deliverables::upsert(c, "speed-edit", "project-plan/plan@v2.0.pdf", 5, "2026-10-02T10:00:00Z", None).unwrap());
    portflow_sync::run(&fake, &m.ledger, &m.settings()).unwrap();
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
    let outcomes = m.read(|c| store::outcomes(c, ME)).unwrap();
    let design = outcomes.iter().find(|o| o.name == "Design").unwrap();
    assert!(design.removed);
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
    assert!(before.keys().any(|k| k.starts_with("portflow/")));

    m.switch(false);
    // The app keeps working: the vault is loaded, edited and written, and the Portflow files stay as they were.
    m.edit(|c| {
        threads::create(c, "speed-edit", "Other").unwrap();
    });
    m.reload();
    let after = m.files();
    for (path, bytes) in &before {
        if path.starts_with("portflow/") || path.contains("/evidence/") || path.contains("/portflow/") || path.contains("/feedback/") || path.contains("/deliverables/") {
            assert_eq!(after.get(path), Some(bytes), "{path} changed while Portflow was off");
        }
    }
    // Nothing Portflow made is in view.
    assert!(m.read(|c| store::outcomes(c, ME)).unwrap().is_empty());
    assert!(m.read(|c| store::evidence_in_project(c, "speed-edit", ME)).unwrap().is_empty());
    // But the links and who gave feedback are kept, in files Ledger always writes.
    let project = fs::read_to_string(m.root().join("projects/speed-edit/project.md")).unwrap();
    assert!(project.contains("portflowCollections"), "{project}");
    let coach = m.read(|c| feedback::list_for_thread(c, "speed-edit", "plan")).unwrap();
    assert_eq!(coach[0].from.as_deref(), Some("Coach Teacher"));

    // Switched on again, it's all back.
    m.switch(true);
    assert_eq!(m.read(|c| store::outcomes(c, ME)).unwrap().len(), 2);
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

    // The session is kept in .ledger/.env, so a restart needs no sign-in.
    let token = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjQxMDI0NDQ4MDB9.sig";
    crate::portflow_auth::save_session(m.root(), &Session { token: token.into(), csrf: Some("c".into()), portfolio: None, expires_at: None }).unwrap();
    let restarted = crate::portflow::PortflowSyncer::new(m.core.clone(), Arc::new(|_| {}), Arc::new(|| {}), Arc::new(|_: &Session, _| Err(PortflowError::Off)), None, Duration::from_secs(3600));
    restarted.reconfigure();
    assert_eq!(restarted.auth.session().map(|s| s.token), Some(token.to_string()));
    assert!(fs::read_to_string(m.root().join(".ledger/.env")).unwrap().contains("PORTFLOW_SESSION="));

    m.switch(false);
    syncer.reconfigure();
    assert!(!syncer.running());
    assert!(syncer.auth.session().is_none(), "switching off drops the session");
    assert!(crate::portflow_auth::load_session(m.root(), 0).is_none(), "and clears the saved one");
    assert_eq!(syncer.status().state, "off");
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
