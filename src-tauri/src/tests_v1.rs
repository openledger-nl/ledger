//! What v1 adds to the vault: a project's kind, goal and end, its people, a post's kind and labels, a deliverable's
//! number and goal, and the label a published version was meant to get. Each is kept in the files it belongs to and
//! loads back as it was written; hand-written values are read leniently.

use std::fs;
use std::path::Path;

use rusqlite::Connection;
use tempfile::tempdir;

use crate::frontmatter::{self, MetaExt};
use crate::project_facts::{self, ProjectMeta, ProjectPerson};
use crate::projects;
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
    let mut vault = Vault::open(root.to_path_buf()).unwrap();
    let conn = vault.load().unwrap();
    (vault, conn)
}

fn study(slug: &str) -> ProjectMeta {
    ProjectMeta {
        slug: slug.into(),
        kind: "study".into(),
        goal: "  Give the bakery a rota tool.  ".into(),
        end: Some("semester".into()),
        semester: Some("NJ26".into()),
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
    let slug = projects::create(&conn, "Thrym Player").unwrap().slug;
    assert_eq!(project_facts::get(&conn, &slug).unwrap(), None);
    assert!(project_facts::list(&conn).unwrap().is_empty());
}

#[test]
fn a_projects_facts_are_kept_in_its_project_file_and_load_back() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let slug = projects::create(&conn, "Thrym Player").unwrap().slug;
    let saved = project_facts::save(&conn, &slug, &study(&slug)).unwrap();
    assert_eq!(saved.goal, "Give the bakery a rota tool.", "trimmed");
    assert_eq!(saved.client, None, "an empty client is none");
    assert_eq!(saved.tag, slug);
    vault.persist(&conn).unwrap();

    let text = read(root, &format!("projects/{slug}/project.md"));
    let meta = frontmatter::parse(&text).unwrap().meta;
    assert_eq!(meta.text("kind").as_deref(), Some("study"));
    assert_eq!(meta.text("end").as_deref(), Some("semester"));
    assert_eq!(meta.text("intendedMmm").as_deref(), Some("cyborg"));
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
        "---\nname: Synth build\nkind: Hobby\ngoal: A working synth.\nend: someday\nintendedMmm: robot\n---\n",
    );
    write(root, "projects/bare/project.md", "---\nname: Bare\nkind: company\n---\n");
    let (_vault, conn) = open(root);
    let synth = project_facts::get(&conn, "synth").unwrap().unwrap();
    assert_eq!(synth.kind, "hobby");
    assert_eq!(synth.end, None, "an end that is no day is no end");
    assert_eq!(synth.intended_mmm, None);
    assert_eq!(project_facts::get(&conn, "bare").unwrap(), None, "a kind without a goal is not enough");
}

// ---- its people ---------------------------------------------------------------------------------------

fn person(name: &str, role: &str) -> ProjectPerson {
    ProjectPerson { name: name.into(), role: role.into(), ..Default::default() }
}

#[test]
fn a_projects_people_are_kept_in_its_project_file_with_or_without_facts() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let slug = projects::create(&conn, "Thrym Player").unwrap().slug;
    let saved = project_facts::save_people(
        &conn,
        &slug,
        &[person(" Frank de Vries ", "coach"), person("Sanne", "teammate"), person("Sanne", "Teammate"), ProjectPerson { portflow_user: Some(4711), ..person("Ries", "client") }],
    )
    .unwrap();
    assert_eq!(saved.iter().map(|p| p.tag.as_str()).collect::<Vec<_>>(), ["coach/frank-de-vries", "teammate/sanne", "client/ries"], "trimmed, once each");
    assert_eq!(project_facts::get(&conn, &slug).unwrap(), None, "people without facts");
    vault.persist(&conn).unwrap();

    let text = read(root, &format!("projects/{slug}/project.md"));
    assert!(text.contains("people:\n- name: Frank de Vries\n  role: coach\n"), "{text}");
    assert!(text.contains("portflowUser: 4711"), "{text}");

    let (_vault, again) = open(root);
    assert_eq!(project_facts::people(&again, Some(&slug)).unwrap(), saved);
    assert_eq!(project_facts::people(&again, None).unwrap().len(), 3);
}

#[test]
fn people_without_a_name_or_role_are_refused_and_read_leniently() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "projects/synth/project.md", "---\nname: Synth\npeople:\n- name: Bo\n  role: wizard\n- role: coach\n- name: Ana\n  role: EXPERT\n---\n");
    let (_vault, conn) = open(root);
    let people = project_facts::people(&conn, Some("synth")).unwrap();
    assert_eq!(people.iter().map(|p| p.tag.as_str()).collect::<Vec<_>>(), ["other/bo", "expert/ana"]);
    assert!(project_facts::save_people(&conn, "synth", &[person("", "coach")]).is_err());
    assert!(project_facts::save_people(&conn, "synth", &[person("Bo", "wizard")]).is_err());
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
