//! The vault picker: starting without a vault, making and opening one, and the list of recent ones.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use crate::projects;
use crate::settings;
use crate::state::{self, Core};

#[test]
fn without_a_vault_nothing_is_written_until_one_is_made() {
    let config = tempdir().unwrap();
    let place = tempdir().unwrap();
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    assert!(!core.open);
    assert!(!core.vault.root().exists(), "the stand-in vault is never created on disk");
    core.reload().unwrap();

    let path = core.create_vault(place.path(), "  School  ").unwrap();
    assert_eq!(path, place.path().join("School"));
    assert!(core.open);
    assert!(path.join(".gitignore").exists());
    let saved = settings::load_config(config.path());
    assert_eq!(saved.vault_path.as_deref(), Some(&*path.display().to_string()));
    assert_eq!(saved.recent_vaults, vec![path.display().to_string()]);
    assert!(!config.path().join("no-vault").exists());
}

#[test]
fn a_new_vault_never_takes_over_a_folder_with_something_in_it() {
    let config = tempdir().unwrap();
    let place = tempdir().unwrap();
    fs::create_dir(place.path().join("Notes")).unwrap();
    fs::write(place.path().join("Notes/todo.md"), "mine").unwrap();
    fs::create_dir(place.path().join("Empty")).unwrap();

    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    assert!(core.create_vault(place.path(), "Notes").is_err());
    assert!(!core.open);
    assert!(core.create_vault(Path::new("relative"), "Notes").is_err());
    core.create_vault(place.path(), "Empty").unwrap();
    assert_eq!(fs::read_to_string(place.path().join("Notes/todo.md")).unwrap(), "mine");
}

#[test]
fn a_vaults_name_is_a_folder_name_every_system_takes() {
    for bad in ["", "   ", ".", "..", "end.", "a/b", "a\\b", "C:", "what?", "\"q\"", "a|b", "tab\there"] {
        assert!(state::vault_name(bad).is_err(), "{bad:?}");
    }
    assert_eq!(state::vault_name(" Semester 3 ").unwrap(), "Semester 3");
    assert_eq!(state::vault_name("ledger-2026").unwrap(), "ledger-2026");
}

#[test]
fn opening_a_folder_loads_what_is_in_it_and_puts_it_first() {
    let config = tempdir().unwrap();
    let a = tempdir().unwrap();
    let b = tempdir().unwrap();
    {
        let mut other = Core::open_at(b.path().to_path_buf(), tempdir().unwrap().path().to_path_buf(), false).unwrap();
        projects::create(&other.conn, "Thesis").unwrap();
        other.vault.persist(&other.conn).unwrap();
    }

    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    assert!(core.open_vault(&a.path().join("missing")).is_err(), "only a folder that is there opens");
    assert!(!a.path().join("missing").exists());
    core.open_vault(a.path()).unwrap();
    assert!(projects::list(&core.conn).unwrap().is_empty());
    core.open_vault(b.path()).unwrap();
    assert_eq!(projects::list(&core.conn).unwrap()[0].slug, "thesis");
    core.open_vault(a.path()).unwrap();

    let recent = settings::load_config(config.path()).recent_vaults;
    assert_eq!(recent, vec![a.path().display().to_string(), b.path().display().to_string()], "no doubles, the latest first");

    settings::forget_vault(config.path(), &b.path().display().to_string()).unwrap();
    assert_eq!(settings::load_config(config.path()).recent_vaults, vec![a.path().display().to_string()]);
    assert!(b.path().join("projects/thesis/project.md").exists(), "forgetting a vault leaves its files");
}

#[test]
fn the_list_of_recent_vaults_is_kept_short() {
    let config = tempdir().unwrap();
    for i in 0..settings::RECENT_VAULTS + 3 {
        settings::remember_vault(config.path(), &PathBuf::from(format!("/vaults/{i}"))).unwrap();
    }
    let recent = settings::load_config(config.path()).recent_vaults;
    assert_eq!(recent.len(), settings::RECENT_VAULTS);
    assert_eq!(recent[0], format!("/vaults/{}", settings::RECENT_VAULTS + 2));
}

#[test]
fn an_older_config_without_recent_vaults_still_reads() {
    let config = tempdir().unwrap();
    fs::write(config.path().join("config.json"), r#"{"vaultPath":"/kept","recentVaults":"not a list"}"#).unwrap();
    let read = settings::load_config(config.path());
    assert_eq!(read.vault_path.as_deref(), Some("/kept"));
    assert!(read.recent_vaults.is_empty());
}
