//! Encrypted vaults: their files are on disk only sealed, under names that say nothing, and open with the passphrase
//! or the recovery key.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::{TempDir, tempdir};

use crate::state::Core;
use crate::{deliverables, posts, projects, settings, store, threads, uploads, vault_crypto};

const PASSPHRASE: &str = "correct horse battery staple";

/// Every file under `root`, with what is in it.
fn files(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let name = format!("{rel}{}", entry.file_name().to_string_lossy());
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), &format!("{name}/"), out);
            } else {
                out.push((name, fs::read(entry.path()).unwrap()));
            }
        }
    }
    let mut out = vec![];
    walk(root, "", &mut out);
    out.sort();
    out
}

fn shows(root: &Path, needle: &str) -> Vec<String> {
    files(root).into_iter().filter(|(name, bytes)| name.contains(needle) || bytes.windows(needle.len()).any(|w| w == needle.as_bytes())).map(|(n, _)| n).collect()
}

struct Machine {
    _place: TempDir,
    config: TempDir,
    path: PathBuf,
    core: Core,
}

/// A new encrypted vault, with a project, a thread, a post, a deliverable and a picture in it.
fn vault_with_things() -> (Machine, String) {
    let place = tempdir().unwrap();
    let config = tempdir().unwrap();
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    let (path, recovery) = core.create_vault(place.path(), "Secret", Some(PASSPHRASE)).unwrap();
    let recovery = recovery.expect("an encrypted vault has a recovery key").to_string();
    let p = projects::create(&core.conn, "Acquisition of Contoso").unwrap().slug;
    let t = threads::create(&core.conn, &p, "Layoffs in Q1").unwrap().slug;
    posts::add(&core.conn, &p, &t, "Only the board knows about this.", Default::default()).unwrap();
    deliverables::create(&core.conn, &p, "Severance plan", "# Severance plan\n\nThree months for everyone.\n").unwrap();
    uploads::add_bytes(&core.conn, &path, &p, &t, "org chart.png", b"\x89PNG not really", "The chart").unwrap();
    core.vault.persist(&core.conn).unwrap();
    (Machine { _place: place, config, path, core }, recovery)
}

#[test]
fn an_encrypted_vault_keeps_nothing_readable_on_disk() {
    let (m, recovery) = vault_with_things();
    assert!(recovery.starts_with("ledger-key-1:"));
    let on_disk = files(&m.path);
    assert!(on_disk.iter().any(|(n, _)| n == "ledger.json"));
    for (name, _) in &on_disk {
        assert!(name == "ledger.json" || (name.starts_with("data/") && name.ends_with(".lgr")), "{name}");
    }
    for secret in ["Contoso", "contoso", "Layoffs", "layoffs", "board knows", "Severance", "severance", "org chart", "PNG not really", "projects"] {
        assert_eq!(shows(&m.path, secret), Vec::<String>::new(), "{secret} shows on disk");
    }
    // The marker says it is encrypted, and how, and nothing that opens it.
    let marker = fs::read_to_string(m.path.join("ledger.json")).unwrap();
    assert!(marker.contains("aes-256-gcm") && marker.contains("argon2id") && !marker.contains(PASSPHRASE), "{marker}");
    // Nor is anything of the vault's kept in the work folder or this machine's settings in plain text.
    assert!(shows(m.config.path(), "board knows").is_empty());
}

#[test]
fn it_reads_back_as_it_was_written() {
    let (mut m, _) = vault_with_things();
    m.core.reload().unwrap();
    let p = &projects::list(&m.core.conn).unwrap()[0];
    assert_eq!(p.name, "Acquisition of Contoso");
    let t = &p.threads[0];
    assert_eq!(posts::list(&m.core.conn, &p.slug, &t.slug).unwrap()[0].body, "Only the board knows about this.");
    let plan = deliverables::get(&m.core.conn, &p.slug, "severance-plan.md").unwrap();
    assert!(plan.body.unwrap().contains("Three months"));
    let store = store::at(&m.path);
    assert_eq!(store.read(&format!("projects/{}/attachments/org chart.png", p.slug)).unwrap(), b"\x89PNG not really");
    assert!(!m.core.vault.changed_on_disk(), "the listing reads what it wrote");
}

#[test]
fn a_new_start_asks_for_the_passphrase_unless_the_keychain_keeps_the_key() {
    let (m, recovery) = vault_with_things();
    let marker = match crate::marker::inspect(&m.path) {
        crate::marker::Found::Vault(marker) => marker,
        other => panic!("{other:?}"),
    };
    settings::remember_vault(m.config.path(), &m.path).unwrap();

    // Kept in the keychain (the default when it was made): the next start opens it.
    vault_crypto::restart_for_test();
    let again = Core::open(m.config.path().to_path_buf()).unwrap();
    assert!(again.open);
    drop(again);

    // Forgotten: the next start is on the picker, and the vault takes the passphrase.
    vault_crypto::forget(&marker.id);
    vault_crypto::restart_for_test();
    let mut locked = Core::open(m.config.path().to_path_buf()).unwrap();
    assert!(!locked.open, "a vault whose key this machine doesn't have isn't opened");
    let header = marker.encryption.clone().unwrap();
    assert!(vault_crypto::unlock(&header, &marker.id, "the wrong words").is_err());
    assert!(vault_crypto::unlock(&header, &marker.id, "ledger-key-1:AAAA").is_err());
    let key = vault_crypto::unlock(&header, &marker.id, PASSPHRASE).unwrap();
    vault_crypto::remember(&marker.id, key, false);
    locked.open_vault(&m.path).unwrap();
    assert_eq!(projects::list(&locked.conn).unwrap()[0].name, "Acquisition of Contoso");

    // The recovery key opens it as well.
    vault_crypto::restart_for_test();
    let key = vault_crypto::unlock(&header, &marker.id, &recovery).unwrap();
    assert_eq!(key.id_hex(), header.key_id);
}

#[test]
fn a_new_passphrase_seals_the_same_key() {
    let (m, _) = vault_with_things();
    let crate::marker::Found::Vault(mut marker) = crate::marker::inspect(&m.path) else { panic!() };
    let header = marker.encryption.clone().unwrap();
    let key = vault_crypto::unlock(&header, &marker.id, PASSPHRASE).unwrap();
    assert!(vault_crypto::rewrap(&header, &marker.id, &key, "short").is_err(), "a passphrase must be long enough");
    let rewrapped = vault_crypto::rewrap(&header, &marker.id, &key, "a whole new set of words").unwrap();
    marker.encryption = Some(rewrapped.clone());
    crate::marker::write(&m.path, &marker).unwrap();
    assert!(vault_crypto::unlock(&rewrapped, &marker.id, PASSPHRASE).is_err(), "the old passphrase no longer opens it");
    let same = vault_crypto::unlock(&rewrapped, &marker.id, "a whole new set of words").unwrap();
    assert_eq!(same.as_bytes(), key.as_bytes(), "the files need no new sealing");
}

#[test]
fn only_the_file_that_changed_is_sealed_again() {
    let (mut m, _) = vault_with_things();
    let before = files(&m.path);
    let p = projects::list(&m.core.conn).unwrap().remove(0);
    let t = &p.threads[0];
    let post = posts::list(&m.core.conn, &p.slug, &t.slug).unwrap().remove(0);
    std::thread::sleep(Duration::from_millis(20));
    posts::update(&m.core.conn, &p.slug, &t.slug, &post.id, "Now the works council knows too.").unwrap();
    assert_eq!(m.core.vault.persist(&m.core.conn).unwrap(), 1);
    let after = files(&m.path);
    let changed: Vec<&String> = after.iter().filter(|f| !before.contains(f)).map(|(n, _)| n).collect();
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert_eq!(before.len(), after.len());
}

#[test]
fn a_copy_the_cloud_made_of_a_sealed_file_is_a_conflict_copy() {
    let (mut m, _) = vault_with_things();
    let p = projects::list(&m.core.conn).unwrap().remove(0);
    let t = p.threads[0].slug.clone();
    let post = posts::list(&m.core.conn, &p.slug, &t).unwrap().remove(0);
    let rel = format!("projects/{}/threads/{t}/posts/{}.md", p.slug, post.id);
    let store = m.core.vault.store().clone();
    // The laptop's version, as OneDrive keeps it beside the original: the same path, sealed, under another name.
    let blob = files(&m.path).into_iter().map(|(n, _)| n).find(|n| store.fingerprint(&rel) == store::fingerprint_of(&m.path.join(n))).unwrap();
    let laptop = m.path.join(blob.replace(".lgr", "-LAPTOP.lgr"));
    fs::copy(m.path.join(&blob), &laptop).unwrap();
    store.write(&rel, format!("---\nid: {}\n---\n\nThe desktop's version.\n", post.id).as_bytes()).unwrap();

    m.core.reload().unwrap();
    let conflicts = m.core.vault.conflicts().to_vec();
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].original, rel);
    assert_eq!(posts::list(&m.core.conn, &p.slug, &t).unwrap().len(), 1, "read in once");
    assert!(String::from_utf8(store.read(&conflicts[0].copy).unwrap()).unwrap().contains("Only the board knows"));

    // Keeping the copy: it replaces the original, and the second sealed file goes.
    let bytes = store.read(&conflicts[0].copy).unwrap();
    store.write(&conflicts[0].original, &bytes).unwrap();
    store.remove(&conflicts[0].copy).unwrap();
    assert!(!laptop.exists());
    m.core.reload().unwrap();
    assert!(m.core.vault.conflicts().is_empty());
    assert_eq!(posts::list(&m.core.conn, &p.slug, &t).unwrap()[0].body, "Only the board knows about this.");
}

#[test]
fn a_program_outside_ledger_works_on_a_plain_copy_that_is_taken_back_in() {
    let (mut m, _) = vault_with_things();
    let p = projects::list(&m.core.conn).unwrap().remove(0);
    let store = m.core.vault.store().clone();
    let rel = format!("projects/{}/deliverables/severance-plan.md", p.slug);
    let file = store.materialize(&rel).unwrap();
    assert!(!file.starts_with(&m.path), "the plain copy is outside the synced folder");
    assert!(fs::read_to_string(&file).unwrap().contains("Three months"));
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&file, "# Severance plan\n\nSix months for everyone.\n").unwrap();
    assert_eq!(store.take_back().unwrap(), 1);
    assert!(shows(&m.path, "Six months").is_empty(), "taken in sealed");
    m.core.reload().unwrap();
    assert!(deliverables::get(&m.core.conn, &p.slug, "severance-plan.md").unwrap().body.unwrap().contains("Six months"));

    // A whole project, as a Claude Code run gets it: what it adds is taken in too.
    let folder = store.materialize_dir(&format!("projects/{}", p.slug)).unwrap();
    assert!(folder.join("deliverables/severance-plan.md").is_file());
    fs::write(folder.join("deliverables/faq.md"), "# FAQ\n\nWho is affected?\n").unwrap();
    store.take_back().unwrap();
    m.core.reload().unwrap();
    assert!(deliverables::get(&m.core.conn, &p.slug, "faq.md").is_ok());
    assert!(shows(&m.path, "Who is affected").is_empty());
}

#[test]
fn a_vault_of_plain_files_gets_an_encrypted_copy_and_is_left_as_it_was() {
    let place = tempdir().unwrap();
    let config = tempdir().unwrap();
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    let (plain, _) = core.create_vault(place.path(), "Open", None).unwrap();
    let p = projects::create(&core.conn, "Bakery rota").unwrap().slug;
    let t = threads::create(&core.conn, &p, "Shifts").unwrap().slug;
    posts::add(&core.conn, &p, &t, "Sunday is the busiest day.", Default::default()).unwrap();
    uploads::add_bytes(&core.conn, &plain, &p, &t, "rota.png", b"a picture", "").unwrap();
    core.vault.persist(&core.conn).unwrap();
    let before = files(&plain);

    assert!(core.encrypted_copy(place.path(), "Open", PASSPHRASE).is_err(), "not into a folder with something in it");
    let (copy, recovery) = core.encrypted_copy(place.path(), "Closed", PASSPHRASE).unwrap();
    assert!(recovery.starts_with("ledger-key-1:"));
    assert_eq!(core.vault.root(), copy);
    assert_eq!(projects::list(&core.conn).unwrap()[0].name, "Bakery rota");
    assert_eq!(store::at(&copy).read(&format!("projects/{p}/attachments/rota.png")).unwrap(), b"a picture");
    assert!(shows(&copy, "busiest").is_empty() && shows(&copy, "Bakery").is_empty());
    assert_eq!(files(&plain), before, "the vault of plain files is left as it is");
    assert!(core.encrypted_copy(place.path(), "Again", PASSPHRASE).is_err(), "an encrypted vault isn't copied again");
}

#[test]
fn a_file_moved_in_an_encrypted_vault_keeps_the_time_it_was_changed() {
    let (m, _) = vault_with_things();
    let store = store::at(&m.path);
    let p = projects::list(&m.core.conn).unwrap()[0].slug.clone();
    let (from, to) = (format!("projects/{p}/attachments/org chart.png"), format!("projects/{p}/attachments/chart.png"));
    let before = store.fingerprint(&from).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    store.rename(&from, &to).unwrap();
    assert!(!store.exists(&from));
    assert_eq!(store.read(&to).unwrap(), b"\x89PNG not really");
    assert_eq!(store.fingerprint(&to).unwrap().1, before.1, "as a move of a plain file keeps it");
}
