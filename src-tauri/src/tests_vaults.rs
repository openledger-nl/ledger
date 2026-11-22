//! The vault picker: starting without a vault, making and opening one, and the list of recent ones.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use crate::{marker, projects};
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

    let (path, recovery) = core.create_vault(place.path(), "  School  ", None).unwrap();
    assert!(recovery.is_none(), "a vault of plain files has no key");
    assert_eq!(path, place.path().join("School"));
    assert!(core.open);
    assert!(path.join("ledger.json").exists(), "a new vault gets its marker");
    // What belongs to this machine is kept by the vault's id in the config folder, never in the vault.
    assert_eq!(settings::local_dir(&path), config.path().join("vaults").join(core.vault.id()));
    settings::update_settings(&path, |s| s.screenshot_hotkey = Some("Alt+F9".into())).unwrap();
    assert!(config.path().join("vaults").join(core.vault.id()).join("settings.json").exists());
    let mut made: Vec<String> = fs::read_dir(&path).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    made.sort();
    assert_eq!(made, ["ledger.json", "shared"], "only the marker and the templates it starts with are in the new vault");
    // It starts with a project plan and NJ26: templates of its own, to change or delete.
    assert!(path.join("shared/templates/prompts/project-plan.md").exists());
    assert!(path.join("shared/templates/semesters/nj26.md").exists(), "and the semester NJ26 to start one from");
    assert_eq!(crate::templates::list(&core.conn, "").unwrap().iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["project-plan"]);
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
    assert!(core.create_vault(place.path(), "Notes", None).is_err());
    assert!(!core.open);
    assert!(core.create_vault(Path::new("relative"), "Notes", None).is_err());
    core.create_vault(place.path(), "Empty", None).unwrap();
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

#[test]
fn a_vault_from_ledger_0x_is_refused_and_left_as_it_is() {
    let config = tempdir().unwrap();
    let old = tempdir().unwrap();
    fs::create_dir_all(old.path().join("projects/thesis")).unwrap();
    fs::write(old.path().join("projects/thesis/project.md"), "---
name: Thesis
---
").unwrap();
    fs::create_dir_all(old.path().join(".ledger")).unwrap();
    fs::write(old.path().join(".ledger/settings.json"), "{}").unwrap();
    let before: Vec<_> = walk(old.path());

    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    let refused = core.open_vault(old.path()).unwrap_err().to_string();
    assert!(refused.contains("Ledger 0.x"), "{refused}");
    assert!(!core.open);
    assert!(!old.path().join(marker::FILE).exists(), "nothing is written into a 0.x vault");
    assert_eq!(walk(old.path()), before);

    // Opened last time, by an older Ledger: the app starts on the picker instead, and the list says why.
    settings::remember_vault(config.path(), old.path()).unwrap();
    let core = Core::open(config.path().to_path_buf()).unwrap();
    assert!(!core.open);
    assert_eq!(walk(old.path()), before);
}

#[test]
fn a_vault_from_a_newer_ledger_or_with_a_damaged_marker_is_refused() {
    let newer = tempdir().unwrap();
    fs::write(newer.path().join(marker::FILE), r#"{"ledger": 99, "id": "abc"}"#).unwrap();
    assert!(crate::vault::Vault::open(newer.path().to_path_buf()).err().unwrap().to_string().contains("newer Ledger"));

    let damaged = tempdir().unwrap();
    fs::write(damaged.path().join(marker::FILE), "{ not json").unwrap();
    assert!(crate::vault::Vault::open(damaged.path().to_path_buf()).is_err());
    assert_eq!(fs::read_to_string(damaged.path().join(marker::FILE)).unwrap(), "{ not json", "a damaged marker is left for the person to see");
}

#[test]
fn a_folder_of_other_notes_becomes_a_vault_and_keeps_its_notes() {
    let config = tempdir().unwrap();
    let notes = tempdir().unwrap();
    fs::write(notes.path().join("ideas.md"), "mine").unwrap();
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    core.open_vault(notes.path()).unwrap();
    assert!(core.open);
    let marker::Found::Vault(found) = marker::inspect(notes.path()) else { panic!("the folder is a vault now") };
    assert_eq!(found.ledger, marker::FORMAT);
    assert_eq!(found.id.len(), 32);
    assert_eq!(core.vault.id(), found.id);
    assert_eq!(fs::read_to_string(notes.path().join("ideas.md")).unwrap(), "mine");

    // Opened again, it is the same vault.
    core.reload().unwrap();
    let again = crate::vault::Vault::open(notes.path().to_path_buf()).unwrap();
    assert_eq!(again.id(), found.id);
}

/// Every file under `root`, with its contents.
fn walk(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn go(dir: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = format!("{rel}/{}", entry.file_name().to_string_lossy());
            if entry.file_type().unwrap().is_dir() {
                go(&entry.path(), &name, out);
            } else {
                out.push((name, fs::read(entry.path()).unwrap()));
            }
        }
    }
    let mut out = vec![];
    go(root, "", &mut out);
    out
}
