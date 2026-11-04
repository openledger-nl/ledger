//! The first-run guide: when it shows, that it is decided once, what it keeps, and that it never costs the config the
//! vault's location.

use std::fs;

use tempfile::tempdir;

use crate::db;
use crate::models::SemesterDraft;
use crate::onboarding::{self, OnboardingConfig, Stage};
use crate::projects;
use crate::semesters;
use crate::settings;

#[test]
fn a_vault_with_nothing_in_it_is_a_new_clients() {
    let dir = tempdir().unwrap();
    let conn = db::open_in_memory().unwrap();
    assert!(!onboarding::has_content(&conn).unwrap());
    let view = onboarding::status(dir.path(), onboarding::has_content(&conn).unwrap()).unwrap();
    assert!(view.active);
    assert!(view.skipped.is_empty());
}

#[test]
fn a_vault_with_a_project_or_a_semester_has_been_set_up_already() {
    let with_project = db::open_in_memory().unwrap();
    projects::create(&with_project, "Pricing").unwrap();
    assert!(onboarding::has_content(&with_project).unwrap());

    let with_semester = db::open_in_memory().unwrap();
    semesters::create(
        &with_semester,
        &SemesterDraft { name: "NJ26".into(), start: "2026-08-31".into(), end: "2027-01-24".into(), sprint_weeks: 3, ..Default::default() },
    )
    .unwrap();
    assert!(onboarding::has_content(&with_semester).unwrap());

    // So a machine that already used Ledger before the guide existed never gets it.
    let dir = tempdir().unwrap();
    let view = onboarding::status(dir.path(), true).unwrap();
    assert!(!view.active);
    assert_eq!(settings::load_config(dir.path()).onboarding.stage, Stage::Done);
}

#[test]
fn the_answer_is_given_once_and_setting_up_the_vault_does_not_change_it() {
    let dir = tempdir().unwrap();
    assert!(onboarding::status(dir.path(), false).unwrap().active);
    // The first project, a semester: the vault has something now, and the guide is only halfway.
    assert!(onboarding::status(dir.path(), true).unwrap().active, "the guide stays until it is finished");
    assert!(!onboarding::finish(dir.path()).unwrap().active);
    assert!(!onboarding::status(dir.path(), false).unwrap().active, "and stays away, even for an empty vault");
}

#[test]
fn the_guide_can_be_asked_for_again_from_the_start() {
    let dir = tempdir().unwrap();
    onboarding::status(dir.path(), true).unwrap();
    onboarding::skip(dir.path(), "sync").unwrap();
    let again = onboarding::restart(dir.path()).unwrap();
    assert!(again.active);
    assert!(again.skipped.is_empty(), "what was put off is asked about again");
    assert!(onboarding::status(dir.path(), true).unwrap().active);
}

#[test]
fn only_the_steps_that_can_wait_can_be_skipped_and_each_is_kept_once() {
    let dir = tempdir().unwrap();
    onboarding::status(dir.path(), false).unwrap();
    assert_eq!(onboarding::skip(dir.path(), "sync").unwrap().skipped, ["sync"]);
    assert_eq!(onboarding::skip(dir.path(), "sync").unwrap().skipped, ["sync"]);
    assert_eq!(onboarding::skip(dir.path(), "semester").unwrap().skipped, ["sync", "semester"]);
    for core in ["thread", "deliverable", "nope", ""] {
        assert!(onboarding::skip(dir.path(), core).is_err(), "{core:?} is not a step that can wait");
    }
    assert_eq!(settings::load_config(dir.path()).onboarding.skipped, ["sync", "semester"]);
}

#[test]
fn the_guide_shares_the_config_file_without_costing_the_vault_its_place() {
    let dir = tempdir().unwrap();
    settings::update_config(dir.path(), |c| c.vault_path = Some("/somewhere/else".into())).unwrap();
    onboarding::status(dir.path(), false).unwrap();
    onboarding::skip(dir.path(), "sync").unwrap();
    onboarding::finish(dir.path()).unwrap();
    let config = settings::load_config(dir.path());
    assert_eq!(config.vault_path.as_deref(), Some("/somewhere/else"));
    assert_eq!(config.onboarding, OnboardingConfig { stage: Stage::Done, skipped: vec!["sync".into()] });

    // And changing the vault's location leaves the guide as it was.
    settings::update_config(dir.path(), |c| c.vault_path = Some("/elsewhere/still".into())).unwrap();
    assert_eq!(settings::load_config(dir.path()).onboarding.stage, Stage::Done);
}

#[test]
fn a_config_edited_into_nonsense_falls_back_without_losing_the_rest() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("config.json"), r#"{"vaultPath":"/kept","onboarding":{"stage":"sideways","skipped":7}}"#).unwrap();
    let config = settings::load_config(dir.path());
    assert_eq!(config.vault_path.as_deref(), Some("/kept"));
    assert_eq!(config.onboarding, OnboardingConfig::default());
    // An old config, from before the guide, has no such key and reads the same way.
    fs::write(dir.path().join("config.json"), r#"{"vaultPath":"/kept"}"#).unwrap();
    assert_eq!(settings::load_config(dir.path()).onboarding.stage, Stage::Undecided);
}
