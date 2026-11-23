//! What v1 adds to the vault: a project's kind, goal and end, its people, a post's kind and labels, a deliverable's
//! number and goal, and the label a published version was meant to get. Each is kept in the files it belongs to and
//! loads back as it was written; hand-written values are read leniently.

use std::fs;
use std::path::Path;

use rusqlite::Connection;
use tempfile::tempdir;

use crate::frontmatter::{self, MetaExt};
use crate::project_facts::{self, ProjectMeta, ProjectPerson};
use crate::deliverable_facts::{self, DeliverableMeta};
use crate::{deliverables, projects};
use crate::vault::Vault;

fn write(root: &Path, rel: &str, text: &str) {
    let file = root.join(rel);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn open(root: &Path) -> (Vault, Connection) {
    crate::marker::ensure(root);
    let mut vault = Vault::open(root.to_path_buf()).unwrap();
    let conn = vault.load().unwrap();
    (vault, conn)
}

/// The semester `study` runs in; `with_semester` makes it.
const NJ26: &str = "nj26";

fn with_semester(conn: &Connection) {
    let draft = crate::models::SemesterDraft { name: "NJ26".into(), start: "2026-08-31".into(), end: "2027-01-24".into(), ..Default::default() };
    crate::semesters::create(conn, &draft).unwrap();
}

fn study(slug: &str) -> ProjectMeta {
    ProjectMeta {
        slug: slug.into(),
        kind: "study".into(),
        goal: "  Give the bakery a rota tool.  ".into(),
        end: Some("semester".into()),
        semester: Some(NJ26.into()),
        intended_mmm: Some("cyborg".into()),
        main_question: Some("How can staff swap shifts?".into()),
        client: Some(String::new()),
        ..Default::default()
    }
}

// ---- a project's facts --------------------------------------------------------------------------------

#[test]
fn a_project_has_no_facts_until_it_has_a_kind_and_a_goal() {
    let dir = tempdir().unwrap();
    let (_vault, conn) = open(dir.path());
    let slug = projects::create(&conn, "Foo Bar").unwrap().slug;
    assert_eq!(project_facts::get(&conn, &slug).unwrap(), None);
    assert!(project_facts::list(&conn).unwrap().is_empty());
}

#[test]
fn a_projects_facts_are_kept_in_its_project_file_and_load_back() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let slug = projects::create(&conn, "Foo Bar").unwrap().slug;
    with_semester(&conn);
    let saved = project_facts::save(&conn, &slug, &study(&slug)).unwrap();
    assert_eq!(saved.goal, "Give the bakery a rota tool.", "trimmed");
    assert_eq!(saved.client, None, "an empty client is none");
    assert_eq!(saved.tag, slug);
    vault.persist(&conn).unwrap();

    let text = read(root, &format!("projects/{slug}/project.md"));
    let meta = frontmatter::parse(&text).unwrap().meta;
    assert_eq!(meta.text("kind").as_deref(), Some("study"));
    assert_eq!(meta.text("end").as_deref(), Some("semester"));
    assert_eq!(meta.text("semester").as_deref(), Some(NJ26), "the semester by its id, which a rename leaves alone");
    assert_eq!(meta.text("intendedAiUse").as_deref(), Some("cyborg"));
    assert_eq!(meta.text("mainQuestion").as_deref(), Some("How can staff swap shifts?"));
    assert!(meta.get("client").is_none(), "{text}");

    let (_vault, again) = open(root);
    assert_eq!(project_facts::get(&again, &slug).unwrap(), Some(saved));
}

#[test]
fn facts_that_make_no_sense_are_refused() {
    let dir = tempdir().unwrap();
    let (_vault, conn) = open(dir.path());
    let slug = projects::create(&conn, "Synth").unwrap().slug;
    let bad = |m: ProjectMeta| project_facts::save(&conn, &slug, &m).is_err();
    assert!(bad(study(&slug)), "a semester that isn't in the vault");
    with_semester(&conn);
    assert!(bad(ProjectMeta { kind: "school".into(), ..study(&slug) }));
    assert!(bad(ProjectMeta { goal: " ".into(), ..study(&slug) }));
    assert!(bad(ProjectMeta { end: Some("next week".into()), ..study(&slug) }));
    assert!(bad(ProjectMeta { intended_mmm: Some("robot".into()), ..study(&slug) }));
    assert!(!bad(ProjectMeta { kind: "hobby".into(), end: Some("2027-03-01".into()), ..study(&slug) }));
}

#[test]
fn hand_written_facts_are_read_leniently() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "projects/synth/project.md",
        "---\nname: Synth build\nkind: Hobby\ngoal: A working synth.\nend: someday\nintendedAiUse: robot\n---\n",
    );
    write(root, "projects/bare/project.md", "---\nname: Bare\nkind: work\n---\n");
    let (_vault, conn) = open(root);
    let synth = project_facts::get(&conn, "synth").unwrap().unwrap();
    assert_eq!(synth.kind, "hobby");
    assert_eq!(synth.end, None, "an end that is no day is no end");
    assert_eq!(synth.intended_mmm, None);
    assert_eq!(project_facts::get(&conn, "bare").unwrap(), None, "a kind without a goal is not enough");
}

// ---- its people ---------------------------------------------------------------------------------------

fn vault_person(conn: &Connection, name: &str) -> String {
    crate::people::create(conn, &crate::people::PersonDraft { name: name.into(), ..Default::default() }).unwrap().id
}

fn attached(person: &str, role: &str) -> ProjectPerson {
    ProjectPerson { person: person.into(), role: role.into(), ..Default::default() }
}

#[test]
fn people_are_the_vaults_and_kept_in_shared_people() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let frank = crate::people::create(
        &conn,
        &crate::people::PersonDraft { name: " Frank de Vries ".into(), email: "f@example.nl".into(), portflow_user: Some(4711), notes: "Coach since 2025.".into(), ..Default::default() },
    )
    .unwrap();
    assert_eq!((frank.id.as_str(), frank.name.as_str(), frank.tag.as_str()), ("frank-de-vries", "Frank de Vries", "people/frank-de-vries"));
    let other = vault_person(&conn, "Frank de Vries");
    assert_eq!(other, "frank-de-vries-2", "a name that is taken gets a number");
    vault.persist(&conn).unwrap();

    let text = read(root, "shared/people/frank-de-vries.md");
    assert!(text.contains("name: Frank de Vries") && text.contains("email: f@example.nl") && text.contains("portflowUser: 4711"), "{text}");
    assert!(text.ends_with("Coach since 2025.\n"), "{text}");

    // Renamed, the id (and so the tag and the file) stays.
    crate::people::update(&conn, "frank-de-vries", &crate::people::PersonDraft { name: "Frank Vries".into(), ..Default::default() }).unwrap();
    vault.persist(&conn).unwrap();
    assert!(read(root, "shared/people/frank-de-vries.md").contains("name: Frank Vries"));

    let (_vault, again) = open(root);
    let people = crate::people::list(&again).unwrap();
    assert_eq!(people.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["frank-de-vries-2", "frank-de-vries"]);
    assert!(crate::people::create(&again, &crate::people::PersonDraft::default()).is_err(), "a person needs a name");
}

#[test]
fn a_person_shows_only_in_the_projects_they_are_attached_to_with_a_role_in_each() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let foo = projects::create(&conn, "Foo Bar").unwrap().slug;
    let bakery = projects::create(&conn, "Bakery").unwrap().slug;
    let quiet = projects::create(&conn, "Quiet").unwrap().slug;
    let frank = vault_person(&conn, "Frank de Vries");
    let sanne = vault_person(&conn, "Sanne");

    let saved = project_facts::save_people(&conn, &foo, &[attached(&frank, "coach"), attached(&sanne, "teammate"), attached(&sanne, "Stakeholder")]).unwrap();
    assert_eq!(saved.iter().map(|p| (p.tag.as_str(), p.role.as_str(), p.name.as_str())).collect::<Vec<_>>(), [("people/frank-de-vries", "coach", "Frank de Vries"), ("people/sanne", "teammate", "Sanne")], "once each");
    project_facts::save_people(&conn, &bakery, &[attached(&frank, "expert")]).unwrap();
    assert_eq!(project_facts::get(&conn, &foo).unwrap(), None, "people without facts");
    assert!(project_facts::people(&conn, Some(&quiet)).unwrap().is_empty(), "nobody is in a project they weren't attached to");
    vault.persist(&conn).unwrap();

    let text = read(root, &format!("projects/{foo}/project.md"));
    assert!(text.contains("people:\n- person: frank-de-vries\n  role: coach\n- person: sanne\n  role: teammate\n"), "{text}");
    assert!(!text.contains("Frank de Vries"), "the name is the person's, kept in their own file: {text}");

    let (mut vault, again) = open(root);
    assert_eq!(project_facts::people(&again, Some(&foo)).unwrap(), saved);
    let frank_now = crate::people::get(&again, &frank).unwrap();
    assert_eq!(frank_now.projects.iter().map(|a| (a.project.as_str(), a.role.as_str())).collect::<Vec<_>>(), [("bakery", "expert"), ("foo-bar", "coach")]);

    // Taken out of the vault, they leave every project.
    crate::people::delete(&again, &frank).unwrap();
    vault.persist(&again).unwrap();
    assert!(!root.join("shared/people/frank-de-vries.md").exists());
    assert!(!read(root, &format!("projects/{bakery}/project.md")).contains("frank"));
    assert_eq!(project_facts::people(&again, None).unwrap().len(), 1);
}

#[test]
fn people_who_arent_in_the_vault_or_have_no_role_are_refused_and_read_leniently() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "shared/people/bo.md", "---\nname: Bo\n---\n");
    write(root, "shared/people/ana.md", "Just notes, no frontmatter.\n");
    write(root, "projects/synth/project.md", "---\nname: Synth\npeople:\n- person: bo\n  role: wizard\n- person: ghost\n  role: coach\n- role: coach\n- person: ana\n  role: EXPERT\n- person: bo\n  role: coach\n---\n");
    let (_vault, conn) = open(root);
    let people = project_facts::people(&conn, Some("synth")).unwrap();
    assert_eq!(people.iter().map(|p| (p.tag.as_str(), p.role.as_str())).collect::<Vec<_>>(), [("people/bo", "other"), ("people/ana", "expert")]);
    assert_eq!(crate::people::get(&conn, "ana").unwrap().name, "ana", "a person without a name is called after the file");
    assert!(project_facts::save_people(&conn, "synth", &[attached("ghost", "coach")]).is_err(), "only people of the vault");
    assert!(project_facts::save_people(&conn, "synth", &[attached("bo", "wizard")]).is_err());
}

// ---- clients ------------------------------------------------------------------------------------------

#[test]
fn a_work_projects_client_is_a_client_of_the_vault_named_by_its_id() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let korenaar = crate::clients::create(&conn, &crate::clients::ClientDraft { name: "Bakkerij De Korenaar".into(), website: "https://korenaar.nl".into(), notes: "Since 1950.".into() }).unwrap();
    assert_eq!(korenaar.id, "bakkerij-de-korenaar");
    let slug = projects::create(&conn, "Rota app").unwrap().slug;
    let work = ProjectMeta { kind: "work".into(), goal: "A rota on phones.".into(), client: Some(korenaar.id.clone()), ..Default::default() };
    let saved = project_facts::save(&conn, &slug, &work).unwrap();
    assert_eq!((saved.client.as_deref(), saved.client_name.as_deref()), (Some("bakkerij-de-korenaar"), Some("Bakkerij De Korenaar")));
    assert!(project_facts::save(&conn, &slug, &ProjectMeta { client: Some("nobody".into()), ..work.clone() }).is_err(), "only a client of the vault");
    vault.persist(&conn).unwrap();

    assert!(read(root, &format!("projects/{slug}/project.md")).contains("client: bakkerij-de-korenaar"));
    let file = read(root, "shared/clients/bakkerij-de-korenaar.md");
    assert!(file.contains("name: Bakkerij De Korenaar") && file.contains("website: https://korenaar.nl") && file.ends_with("Since 1950.\n"), "{file}");

    let (mut vault, again) = open(root);
    assert_eq!(crate::clients::get(&again, "bakkerij-de-korenaar").unwrap().projects.len(), 1);
    // Taken out of the vault, the project keeps going without a client.
    crate::clients::delete(&again, "bakkerij-de-korenaar").unwrap();
    vault.persist(&again).unwrap();
    assert_eq!(project_facts::get(&again, &slug).unwrap().unwrap().client, None);
    assert!(!root.join("shared/clients/bakkerij-de-korenaar.md").exists());
    assert!(!read(root, &format!("projects/{slug}/project.md")).contains("client:"));
}

#[test]
fn a_client_that_isnt_in_the_vault_is_read_as_none() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "projects/rota/project.md", "---\nname: Rota\nkind: work\ngoal: Shifts.\nclient: gone\n---\n");
    let (_vault, conn) = open(root);
    assert_eq!(project_facts::get(&conn, "rota").unwrap().unwrap().client, None);
}

// ---- a post's kind and labels ---------------------------------------------------------------------------

#[test]
fn a_posts_kind_and_labels_are_kept_in_its_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let project = projects::create(&conn, "Acme").unwrap().slug;
    let thread = crate::threads::create(&conn, &project, "Alpha").unwrap().slug;
    let post = crate::posts::add(&conn, &project, &thread, "The API answers in 40 ms.", Default::default()).unwrap();
    assert_eq!((post.kind.clone(), post.labels.clone()), (None, vec![]), "nothing until you say");

    let set = crate::posts::set_meta(&conn, &project, &thread, &post.id, Some("Research"), &["assumption".into(), "Fact".into(), "fact".into()]).unwrap();
    assert_eq!(set.kind.as_deref(), Some("research"));
    assert_eq!(set.labels, ["fact", "assumption"], "each once, in their own order");
    assert!(crate::posts::set_meta(&conn, &project, &thread, &post.id, Some("rant"), &[]).is_err());
    assert!(crate::posts::set_meta(&conn, &project, &thread, &post.id, None, &["maybe".into()]).is_err());
    vault.persist(&conn).unwrap();

    let file = format!("projects/{project}/threads/{thread}/posts/{}.md", post.id);
    let meta = frontmatter::parse(&read(root, &file)).unwrap().meta;
    assert_eq!(meta.text("kind").as_deref(), Some("research"));
    assert_eq!(meta.list("labels"), ["fact", "assumption"]);

    // Hand-edited: an unknown label is dropped, a single label reads as a list of one.
    let text = read(root, &file).replace("labels:\n- fact\n- assumption\n", "labels: To-Check\n").replace("kind: research", "kind: Devlog");
    write(root, &file, &text);
    let (_vault, again) = open(root);
    let loaded = crate::posts::list(&again, &project, &thread).unwrap().remove(0);
    assert_eq!((loaded.kind.as_deref(), loaded.labels), (Some("devlog"), vec!["to-check".to_string()]));
}

// ---- a deliverable's facts --------------------------------------------------------------------------

fn facts(project: &str, path: &str, number: i64) -> DeliverableMeta {
    DeliverableMeta {
        project: project.into(),
        path: path.into(),
        number,
        goal: " Agree on what is built first. ".into(),
        mmm: Some("cyborg".into()),
        answers: Some("main".into()),
        template: Some("plan".into()),
    }
}

#[test]
fn a_deliverables_facts_are_kept_in_its_project_file_and_load_back() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let project = projects::create(&conn, "Acme").unwrap().slug;
    assert_eq!(deliverable_facts::next_number(&conn, &project).unwrap(), 1);
    let plan = deliverables::create(&conn, &project, "01 Project plan.md", "# Plan").unwrap().path;
    let saved = deliverable_facts::save(&conn, &facts(&project, &plan, 1)).unwrap();
    assert_eq!(saved.goal, "Agree on what is built first.");
    assert!(deliverable_facts::save(&conn, &facts(&project, "missing.md", 2)).is_err(), "only for a deliverable that is there");
    assert!(deliverable_facts::save(&conn, &DeliverableMeta { mmm: Some("robot".into()), ..facts(&project, &plan, 1) }).is_err());
    vault.persist(&conn).unwrap();

    let meta = frontmatter::parse(&read(root, &format!("projects/{project}/project.md"))).unwrap().meta;
    assert_eq!(meta.text("nextDeliverable").as_deref(), Some("2"));
    assert_eq!(meta.maps("deliverables")[0].text("path").as_deref(), Some("01-project-plan.md"));

    let (_vault, again) = open(root);
    assert_eq!(deliverable_facts::get(&again, &project, &plan).unwrap(), Some(saved));
    assert_eq!(deliverable_facts::next_number(&again, &project).unwrap(), 2);
}

#[test]
fn a_deliverables_evidence_description_is_kept_follows_a_rename_and_goes_with_it() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let project = projects::create(&conn, "Acme").unwrap().slug;
    let plan = deliverables::create(&conn, &project, "Plan.md", "# Plan").unwrap().path;
    assert_eq!(deliverable_facts::description(&conn, &project, &plan).unwrap().description, "", "none yet");
    assert!(deliverable_facts::save_description(&conn, &project, "missing.md", "x").is_err(), "only for a deliverable that is there");

    let saved = deliverable_facts::save_description(&conn, &project, &plan, "**What?** A plan.\n\n**Goal?** Agree.\n\n**Why it contributes?** Analysing.").unwrap();
    assert_eq!(saved.description, "**Evidence description**\n\n**What?** A plan.\n\n**Goal?** Agree.\n\n**Why it contributes?** Analysing.");
    assert_eq!(saved.description_parts.as_ref().map(|p| p.goal.as_str()), Some("Agree."));
    vault.persist(&conn).unwrap();
    // A note of its own beside the deliverable, not in project.md.
    let file = read(root, &format!("projects/{project}/deliverables/plan.desc.md"));
    assert!(file.starts_with("---\ndeliverable: plan.md\n---\n") && file.contains("**Goal?** Agree."), "{file}");
    assert!(!read(root, &format!("projects/{project}/project.md")).contains("Evidence description"));

    // It loads back, is no deliverable of its own, and follows the deliverable when it is renamed.
    let (mut vault, conn) = open(root);
    assert_eq!(deliverable_facts::description(&conn, &project, &plan).unwrap(), saved);
    assert_eq!(deliverables::list(&conn, &project).unwrap().iter().map(|d| d.path.as_str()).collect::<Vec<_>>(), ["plan.md"]);
    let renamed = deliverables::rename(&conn, root, &project, &plan, "Project plan").unwrap();
    assert_eq!(deliverable_facts::description(&conn, &project, &renamed).unwrap().description, saved.description);
    assert_eq!(deliverable_facts::description(&conn, &project, &plan).unwrap().description, "");
    assert!(deliverables::rename(&conn, root, &project, &renamed, "Notes.desc").is_err(), "a description's name is no deliverable's");
    vault.persist(&conn).unwrap();
    assert!(root.join(format!("projects/{project}/deliverables/Project plan.desc.md")).is_file());
    assert!(!root.join(format!("projects/{project}/deliverables/plan.desc.md")).exists());

    // Emptied, it goes; and deleting the deliverable takes it along.
    assert_eq!(deliverable_facts::save_description(&conn, &project, &renamed, "  ").unwrap().description, "");
    deliverable_facts::save_description(&conn, &project, &renamed, "Free text.").unwrap();
    vault.persist(&conn).unwrap();
    deliverables::delete(&conn, root, &project, &renamed).unwrap();
    assert_eq!(deliverable_facts::description(&conn, &project, &renamed).unwrap().description, "");
    vault.persist(&conn).unwrap();
    assert!(!root.join(format!("projects/{project}/deliverables/Project plan.desc.md")).exists());
}

#[test]
fn a_hand_written_description_is_its_deliverables_by_its_name() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "projects/acme/project.md", "---\nname: Acme\n---\n");
    write(root, "projects/acme/deliverables/reports/plan.pdf", "%PDF");
    write(root, "projects/acme/deliverables/reports/plan.desc.md", "**What?** The plan, as a PDF.\n");
    write(root, "projects/acme/deliverables/ghost.desc.md", "Of nothing that is there.\n");
    let (vault, conn) = open(root);
    let view = deliverable_facts::description(&conn, "acme", "reports/plan.pdf").unwrap();
    assert!(view.description.contains("The plan, as a PDF."), "{view:?}");
    assert_eq!(deliverables::list(&conn, "acme").unwrap().len(), 1);
    assert!(root.join("projects/acme/deliverables/ghost.desc.md").is_file(), "left alone");

    // A second deliverable of the same name: a description that doesn't say whose it is could be either's, and is left
    // alone. Ledger's own say whose they are, and are named in full.
    write(root, "projects/acme/deliverables/reports/plan.md", "# Plan\n");
    let (mut again, reloaded) = open(root);
    assert_eq!(deliverable_facts::description(&reloaded, "acme", "reports/plan.pdf").unwrap().description, "");
    deliverable_facts::save_description(&reloaded, "acme", "reports/plan.md", "**What?** The plan.").unwrap();
    deliverable_facts::save_description(&reloaded, "acme", "reports/plan.pdf", "**What?** The plan, as a PDF.").unwrap();
    again.persist(&reloaded).unwrap();
    let names = |dir: &str| {
        let mut n: Vec<String> = std::fs::read_dir(root.join(dir)).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        n.sort();
        n
    };
    assert_eq!(names("projects/acme/deliverables/reports"), ["plan.desc.md", "plan.md", "plan.md.desc.md", "plan.pdf", "plan.pdf.desc.md"]);
    drop(vault);
    let (_, last) = open(root);
    assert!(deliverable_facts::description(&last, "acme", "reports/plan.pdf").unwrap().description.contains("as a PDF"));
    assert!(deliverable_facts::description(&last, "acme", "reports/plan.md").unwrap().description.contains("The plan."));
}

#[test]
fn a_deliverables_number_is_never_given_twice() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (_vault, conn) = open(root);
    let project = projects::create(&conn, "Acme").unwrap().slug;
    let plan = deliverables::create(&conn, &project, "01 Plan.md", "# Plan").unwrap().path;
    deliverable_facts::save(&conn, &facts(&project, &plan, 1)).unwrap();
    let old = deliverables::create(&conn, &project, "07 Old notes.md", "# Notes").unwrap().path;
    assert_eq!(deliverable_facts::next_number(&conn, &project).unwrap(), 8, "past a numbered file already there");

    deliverables::delete(&conn, root, &project, &plan).unwrap();
    assert_eq!(deliverable_facts::get(&conn, &project, &plan).unwrap(), None, "deleting drops the facts");
    deliverables::delete(&conn, root, &project, &old).unwrap();
    assert_eq!(deliverable_facts::next_number(&conn, &project).unwrap(), 2, "a deleted one's number stays given");
}

#[test]
fn facts_follow_a_deliverable_renamed_outside_ledger() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let project = projects::create(&conn, "Acme").unwrap().slug;
    let plan = deliverables::create(&conn, &project, "01 Plan.md", "# Plan").unwrap().path;
    deliverable_facts::save(&conn, &facts(&project, &plan, 1)).unwrap();
    vault.persist(&conn).unwrap();
    drop((vault, conn));

    let folder = root.join(format!("projects/{project}/deliverables"));
    fs::rename(folder.join(&plan), folder.join("01 Project plan.md")).unwrap();
    let (_vault, conn) = open(root);
    let moved = deliverable_facts::get(&conn, &project, "01 Project plan.md").unwrap().expect("the facts followed the file");
    assert_eq!(moved.number, 1);
    assert_eq!(deliverable_facts::get(&conn, &project, &plan).unwrap(), None);
}

#[test]
fn hand_written_deliverable_facts_are_read_leniently() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "projects/acme/project.md",
        "---\nname: Acme\ndeliverables:\n- path: 03 Brief.md\n  goal: A brief.\n  aiUse: robot\n- goal: No path.\n- path: Loose.md\n  goal: Loose.\n---\n",
    );
    write(root, "projects/acme/deliverables/03 Brief.md", "# Brief\n");
    write(root, "projects/acme/deliverables/Loose.md", "# Loose\n");
    let (_vault, conn) = open(root);
    let list = deliverable_facts::list(&conn, "acme").unwrap();
    assert_eq!(list.iter().map(|f| (f.path.as_str(), f.number)).collect::<Vec<_>>(), [("03 Brief.md", 3), ("Loose.md", 4)]);
    assert_eq!(list[0].mmm, None);
    assert_eq!(deliverable_facts::next_number(&conn, "acme").unwrap(), 5);
}

// ---- the label a published version was meant to get -------------------------------------------------

#[test]
fn a_version_published_with_another_label_keeps_the_intended_one_and_why() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let project = projects::create(&conn, "Acme").unwrap().slug;
    let plan = deliverables::create(&conn, &project, "Plan.md", "# Plan").unwrap().path;
    let published = deliverables::publish(&conn, root, &project, &plan, crate::models::Bump::Minor, None).unwrap();
    let version = published.versions.last().unwrap().clone();

    crate::provenance::confirm_label(&conn, &project, &plan, &version, "man-made", "  I wrote it myself.  ").unwrap();
    assert!(crate::provenance::confirm_label(&conn, &project, &plan, "9.9", "man-made", "").is_err(), "only a version that is there");
    assert!(crate::provenance::confirm_label(&conn, &project, &plan, &version, "robot", "").is_err());
    vault.persist(&conn).unwrap();

    let manifest = root.join(format!("projects/{project}/{}", deliverables::manifest_in_project(&plan, &version, [])));
    let text = fs::read_to_string(&manifest).unwrap();
    let meta = frontmatter::parse(&text).unwrap().meta;
    assert_eq!(meta.text("intendedAiUse").as_deref(), Some("man-made"));
    assert_eq!(meta.text("labelReason").as_deref(), Some("I wrote it myself."));

    // It loads back, and is written again the same.
    let (mut vault, again) = open(root);
    vault.persist(&again).unwrap();
    assert_eq!(fs::read_to_string(&manifest).unwrap(), text);
}

// ---- feedback recorded from the inbox -------------------------------------------------------------------

use crate::feedback::{self, FeedbackRecord};
use crate::models::{FeedbackFrom, StoryDraft};
use crate::{stories, threads};

fn someone(project: &str, name: &str, role: &str) -> FeedbackRecord {
    FeedbackRecord {
        project: project.into(),
        kind: "someone".into(),
        from: FeedbackFrom { name: Some(name.into()), role: Some(role.into()), given: Some("meeting".into()), ..Default::default() },
        body: "Split the plan into sprints.".into(),
        ..Default::default()
    }
}

#[test]
fn feedback_on_a_project_without_threads_starts_a_feedback_thread() {
    let dir = tempdir().unwrap();
    let (_vault, conn) = open(dir.path());
    let project = projects::create(&conn, "Bakery").unwrap().slug;
    let saved = feedback::record(&conn, &someone(&project, "Frank", "coach")).unwrap();
    assert_eq!(saved.kind, "someone");
    assert_eq!(saved.deliverable_path, "");
    let thread = threads::list(&conn, &project).unwrap();
    assert_eq!(thread.len(), 1);
    assert_eq!(thread[0].title, "Feedback");
    assert_eq!(saved.thread_slug, thread[0].slug);
}

#[test]
fn feedback_goes_to_the_most_recently_active_thread_unless_one_is_named() {
    let dir = tempdir().unwrap();
    let (_vault, conn) = open(dir.path());
    let project = projects::create(&conn, "Bakery").unwrap().slug;
    let first = threads::create(&conn, &project, "Research").unwrap().slug;
    let second = threads::create(&conn, &project, "Planning").unwrap().slug;
    let picked = feedback::record(&conn, &FeedbackRecord { thread: Some(first.clone()), ..someone(&project, "Frank", "coach") }).unwrap();
    assert_eq!(picked.thread_slug, first);
    // The thread that just got feedback is now the most recently active one.
    let next = feedback::record(&conn, &someone(&project, "Ries", "client")).unwrap();
    assert_eq!(next.thread_slug, first);
    assert_ne!(next.thread_slug, second);
}

#[test]
fn recorded_feedback_keeps_whose_it_is_its_story_and_its_meeting_in_its_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let project = projects::create(&conn, "Bakery").unwrap().slug;
    let thread = threads::create(&conn, &project, "Planning").unwrap().slug;
    stories::create(&conn, &project, StoryDraft { id: "US-1".into(), summary: "Swap a shift".into(), ..Default::default() }).unwrap();
    let on_story = feedback::record(
        &conn,
        &FeedbackRecord { story: Some("US-1".into()), daily: Some("2026-11-09".into()), at: Some("2026-11-09T14:05:00+01:00".into()), ..someone(&project, "Frank", "coach") },
    )
    .unwrap();
    let own = feedback::record(&conn, &FeedbackRecord { kind: "own".into(), from: FeedbackFrom::default(), body: "Check the rota rules.".into(), ..someone(&project, "", "") }).unwrap();
    assert_eq!(own.kind, "own");
    assert_eq!(own.from, None);
    assert_eq!(on_story.created_at, "2026-11-09T13:05:00.000Z");
    vault.persist(&conn).unwrap();

    let text = read(root, &format!("projects/{project}/threads/{thread}/feedback/{}.md", on_story.id));
    let meta = frontmatter::parse(&text).unwrap().meta;
    assert_eq!(meta.text("kind").as_deref(), Some("someone"));
    assert_eq!(meta.text("story").as_deref(), Some("US-1"));
    assert_eq!(meta.text("daily").as_deref(), Some("2026-11-09"));
    assert_eq!(meta.text("given").as_deref(), Some("meeting"));
    assert_eq!(meta.text("deliverable"), None);

    let (_again, conn) = open(root);
    let inbox = feedback::list_all(&conn).unwrap();
    assert_eq!(inbox.len(), 2);
    let story = inbox.iter().find(|f| f.feedback.id == on_story.id).unwrap();
    assert_eq!(story.feedback.story.as_deref(), Some("US-1"));
    assert_eq!(story.story_title.as_deref(), Some("Swap a shift"));
    assert_eq!(story.feedback.daily.as_deref(), Some("2026-11-09"));
    assert_eq!(story.feedback.from.as_deref(), Some("Frank"));
    assert_eq!(story.thread_title, "Planning");
    assert!(inbox.iter().any(|f| f.feedback.id == own.id && f.feedback.kind == "own"));
}

#[test]
fn someone_elses_feedback_must_say_who_and_lands_on_one_thing() {
    let dir = tempdir().unwrap();
    let (_vault, conn) = open(dir.path());
    let project = projects::create(&conn, "Bakery").unwrap().slug;
    let plan = deliverables::create(&conn, &project, "Plan.md", "# Plan").unwrap().path;
    stories::create(&conn, &project, StoryDraft { id: "US-1".into(), summary: "Swap".into(), ..Default::default() }).unwrap();
    assert!(feedback::record(&conn, &FeedbackRecord { from: FeedbackFrom::default(), ..someone(&project, "", "") }).is_err());
    let both = FeedbackRecord { deliverable: Some(plan.clone()), story: Some("US-1".into()), ..someone(&project, "Frank", "coach") };
    assert!(feedback::record(&conn, &both).is_err());
    assert!(feedback::record(&conn, &FeedbackRecord { deliverable: Some("Nope.md".into()), ..someone(&project, "Frank", "coach") }).is_err());
    let on_plan = feedback::record(&conn, &FeedbackRecord { deliverable: Some(plan.clone()), ..someone(&project, "Frank", "coach") }).unwrap();
    assert_eq!(on_plan.deliverable_path, plan);
}

#[test]
fn hand_written_feedback_without_a_deliverable_reads_in_when_it_says_whose_it_is() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "projects/bakery/project.md", "---\nname: Bakery\n---\n");
    write(root, "projects/bakery/threads/notes/thread.md", "---\ntitle: Notes\n---\n");
    write(root, "projects/bakery/threads/notes/feedback/a.md", "---\nkind: own\ndaily: not a day\n---\n\nRemember the holidays.\n");
    write(root, "projects/bakery/threads/notes/feedback/b.md", "---\nkind: shouting\n---\n\nNo deliverable, no kind: not feedback.\n");
    write(root, "projects/bakery/threads/notes/feedback/c.md", "---\ndeliverable: Plan.md\nfrom: Frank\n---\n\nOlder feedback names its giver.\n");
    let (_vault, conn) = open(root);
    let inbox = feedback::list_all(&conn).unwrap();
    let ids: Vec<&str> = inbox.iter().map(|f| f.feedback.id.as_str()).collect();
    assert!(ids.contains(&"a") && ids.contains(&"c") && !ids.contains(&"b"), "{ids:?}");
    let a = inbox.iter().find(|f| f.feedback.id == "a").unwrap();
    assert_eq!(a.feedback.kind, "own");
    assert_eq!(a.feedback.daily, None, "a day that isn't one is dropped");
    assert_eq!(inbox.iter().find(|f| f.feedback.id == "c").unwrap().feedback.kind, "someone");
}

#[test]
fn the_inbox_holds_every_project_newest_first() {
    let dir = tempdir().unwrap();
    let (_vault, conn) = open(dir.path());
    let a = projects::create(&conn, "Bakery").unwrap().slug;
    let b = projects::create(&conn, "Synth").unwrap().slug;
    let older = feedback::record(&conn, &FeedbackRecord { at: Some("2026-10-01T09:00:00Z".into()), ..someone(&a, "Frank", "coach") }).unwrap();
    let newer = feedback::record(&conn, &FeedbackRecord { at: Some("2026-11-01T09:00:00Z".into()), ..someone(&b, "Ries", "client") }).unwrap();
    let inbox = feedback::list_all(&conn).unwrap();
    assert_eq!(inbox.iter().map(|f| f.feedback.id.clone()).collect::<Vec<_>>(), vec![newer.id, older.id]);
    assert_eq!(inbox[0].project_name, "Synth");
}

// ---- meeting notes: daily notes in meetings/ -------------------------------------------------------------

use crate::daily::{self, DailyDraft, MeetingSection};

fn meeting(time: &str, title: &str, projects: &[&str], people: &[&str], body: &str) -> MeetingSection {
    MeetingSection {
        time: (!time.is_empty()).then(|| time.to_string()),
        title: title.into(),
        projects: projects.iter().map(|s| s.to_string()).collect(),
        people: people.iter().map(|s| s.to_string()).collect(),
        body: body.into(),
    }
}

#[test]
fn a_days_meetings_are_kept_in_its_daily_note_and_load_back() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let frank = vault_person(&conn, "Frank de Vries");
    let draft = DailyDraft {
        date: "2026-11-09".into(),
        meetings: vec![
            meeting("9:00", "Coach meeting", &["foo-bar"], &[&frank, "people/sam-de-wit"], "Split the plan.\n\n- [ ] Ask about sprints"),
            meeting("", "  Stand-up  ", &["foo-bar", "bakery"], &["Sam de Wit"], ""),
        ],
    };
    let saved = daily::save(&conn, &draft).unwrap().unwrap();
    assert_eq!(saved.id, "daily-2026-11-09");
    assert_eq!(saved.meetings[0].time.as_deref(), Some("09:00"));
    assert_eq!(saved.meetings[0].people, vec!["frank-de-vries", "sam-de-wit"], "by their id");
    assert_eq!(saved.meetings[1].people, vec!["sam-de-wit"]);
    assert_eq!(crate::people::get(&conn, "sam-de-wit").unwrap().name, "Sam De Wit", "someone new is added to the vault, once");
    assert_eq!(crate::people::list(&conn).unwrap().len(), 2);
    assert_eq!(saved.meetings[1].title, "Stand-up");
    vault.persist(&conn).unwrap();

    let text = read(root, "meetings/2026/11/2026-11-09.md");
    let meta = frontmatter::parse(&text).unwrap().meta;
    assert!(meta.get("type").is_none() && meta.get("id").is_none(), "a day's note is known by its file's name: {text}");
    assert_eq!(meta.text("date").as_deref(), Some("2026-11-09"));
    assert_eq!(meta.list("tags"), vec!["project/foo-bar", "project/bakery"], "who was there is linked, not tagged");
    // Each meeting's time and projects are in the frontmatter; its heading is its title.
    let listed = meta.maps("meetings");
    assert_eq!((listed[0].text("title").as_deref(), listed[0].text("time").as_deref(), listed[0].list("projects")), (Some("Coach meeting"), Some("09:00"), vec!["foo-bar".to_string()]));
    assert_eq!((listed[1].text("title").as_deref(), listed[1].text("time"), listed[1].list("projects")), (Some("Stand-up"), None, vec!["foo-bar".to_string(), "bakery".to_string()]));
    assert!(text.contains("time: 09:00"), "{text}");
    assert!(text.contains("## Coach meeting\nWith: [[people/frank-de-vries]], [[people/sam-de-wit]]\n\nSplit the plan."), "{text}");
    assert!(text.contains("## Stand-up\nWith: [[people/sam-de-wit]]"), "{text}");

    let (mut again, conn) = open(root);
    let back = daily::get(&conn, "2026-11-09").unwrap().unwrap();
    assert_eq!(back.meetings, saved.meetings);
    again.persist(&conn).unwrap();
    assert_eq!(read(root, "meetings/2026/11/2026-11-09.md"), text, "written back the same");
}

#[test]
fn an_obsidian_daily_note_without_frontmatter_reads_in_and_keeps_its_own_text() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "meetings/2026/11/2026-11-10.md", "Rainy day.\n\n## Client call\nwith: [[people/ries|Ries]]\nThey like it.\n### Next\nShip it.\n\n## Walk\nWith: nobody\n");
    write(root, "meetings/2026/11/2026-11-11.md", "---\ntags: [journal, coach/old]\nmood: good\n---\n\nJust a journal entry.\n");
    write(root, "meetings/notes about days.md", "Not a day.\n");
    let (mut vault, conn) = open(root);
    let note = daily::get(&conn, "2026-11-10").unwrap().unwrap();
    // A `With:` line that links to nobody is part of the notes.
    assert_eq!(note.meetings, vec![meeting("", "Client call", &[], &["ries"], "They like it.\n### Next\nShip it."), meeting("", "Walk", &[], &[], "With: nobody")]);
    assert_eq!(daily::get(&conn, "2026-11-11").unwrap().unwrap().meetings, vec![]);

    // Saving the day again keeps the text before the meetings; emptying a journal day keeps its own text and keys.
    let mut changed = note.meetings.clone();
    changed[0].body = "They love it.".into();
    changed[0].time = Some("14:00".into());
    changed[0].projects = vec!["bakery".into()];
    daily::save(&conn, &DailyDraft { date: "2026-11-10".into(), meetings: changed }).unwrap();
    daily::save(&conn, &DailyDraft { date: "2026-11-11".into(), meetings: vec![] }).unwrap();
    vault.persist(&conn).unwrap();
    let ten = read(root, "meetings/2026/11/2026-11-10.md");
    assert!(ten.contains("Rainy day.\n\n## Client call\nWith: [[people/ries]]\n\nThey love it."), "{ten}");
    assert_eq!(crate::people::get(&conn, "ries").unwrap().name, "Ries", "saving the note gives Ries a file");
    assert!(ten.contains("meetings:\n- title: Client call\n  time: 14:00\n  projects:\n  - bakery\n"), "{ten}");
    // Nothing about the journal day changed, so its file stays as it was written by hand.
    assert_eq!(read(root, "meetings/2026/11/2026-11-11.md"), "---\ntags: [journal, coach/old]\nmood: good\n---\n\nJust a journal entry.\n");
    // Once it gets a meeting it is written by Ledger: its own text, keys and plain tags stay, a tag with levels goes.
    daily::save(&conn, &DailyDraft { date: "2026-11-11".into(), meetings: vec![meeting("", "Walk", &[], &[], "")] }).unwrap();
    vault.persist(&conn).unwrap();
    let eleven = read(root, "meetings/2026/11/2026-11-11.md");
    assert!(eleven.contains("mood: good") && eleven.contains("Just a journal entry.\n\n## Walk") && eleven.contains("- journal"), "{eleven}");
    assert!(!eleven.contains("coach/old"), "{eleven}");
    assert_eq!(read(root, "meetings/notes about days.md"), "Not a day.\n", "a file that isn't a day is left alone");
}

#[test]
fn a_meetings_time_and_projects_are_found_by_its_heading() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // Two stand-ups go with their entries in order; a heading the frontmatter doesn't list has no time or project, and
    // an entry without a heading is nothing. Tags in a heading are words of its title.
    write(
        root,
        "meetings/2026/11/2026-11-12.md",
        "---\nmeetings:\n- title: Stand-up\n  time: '9:00'\n  projects: [bakery]\n- title: Gone\n  time: '11:00'\n- title: stand-up\n  time: '16:00'\n  projects: foo-bar\n---\n\n## Stand-up\n\n## Lunch #food\n\n## Stand-up\nWith: [[people/sam]]\n",
    );
    let (_vault, conn) = open(root);
    let note = daily::get(&conn, "2026-11-12").unwrap().unwrap();
    assert_eq!(
        note.meetings,
        vec![
            meeting("09:00", "Stand-up", &["bakery"], &[], ""),
            meeting("", "Lunch #food", &[], &[], ""),
            meeting("16:00", "Stand-up", &["foo-bar"], &["sam"], ""),
        ]
    );
}

#[test]
fn a_day_without_meetings_has_no_note() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    daily::save(&conn, &DailyDraft { date: "2026-11-09".into(), meetings: vec![meeting("10:00", "Coach", &[], &[], "")] }).unwrap();
    vault.persist(&conn).unwrap();
    assert!(root.join("meetings/2026/11/2026-11-09.md").exists());
    assert_eq!(daily::save(&conn, &DailyDraft { date: "2026-11-09".into(), meetings: vec![] }).unwrap(), None);
    vault.persist(&conn).unwrap();
    assert!(!root.join("meetings/2026/11/2026-11-09.md").exists());
    assert!(daily::save(&conn, &DailyDraft { date: "9-11-2026".into(), meetings: vec![] }).is_err());
}

#[test]
fn daily_notes_are_found_by_project_and_by_person_newest_first() {
    let dir = tempdir().unwrap();
    let (_vault, conn) = open(dir.path());
    let day = |date: &str, m: MeetingSection| daily::save(&conn, &DailyDraft { date: date.into(), meetings: vec![m] }).unwrap();
    day("2026-11-01", meeting("10:00", "Coach", &["foo-bar"], &["frank"], ""));
    day("2026-11-03", meeting("10:00", "Client", &["bakery"], &["ries"], ""));
    day("2026-11-05", meeting("10:00", "Team", &["foo-bar"], &["sam"], ""));
    let dates = |notes: Vec<daily::DailyNote>| notes.into_iter().map(|n| n.date).collect::<Vec<_>>();
    assert_eq!(dates(daily::list(&conn, None, None).unwrap()), vec!["2026-11-05", "2026-11-03", "2026-11-01"]);
    assert_eq!(dates(daily::list(&conn, Some("foo-bar"), None).unwrap()), vec!["2026-11-05", "2026-11-01"]);
    assert_eq!(dates(daily::list(&conn, None, Some("ries")).unwrap()), vec!["2026-11-03"]);
    assert_eq!(dates(daily::list(&conn, Some("bakery"), Some("frank")).unwrap()), Vec::<String>::new());
}
