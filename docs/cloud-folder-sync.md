# A synced folder instead of GitHub

Research, 9 Nov 2026. Nothing here has been built.

The question: can Ledger stop syncing to GitHub and keep the vault in a folder that OneDrive, SharePoint or Nextcloud
keeps in sync instead, so that backing up and restoring become the provider's job? And what can the app do to make
that work well?

## In short

Yes. The vault is already the right shape for it: plain files, with the database only in memory, so the folder holds
no database file that a sync client could tear in half. What has to change is an assumption. Between syncs, Ledger acts
as if it were the only one writing to the vault. GitHub refuses a push made from an out-of-date copy. A sync client
refuses nothing: when two machines change the same file, it keeps both under different names. So Ledger has to notice
outside changes and conflicts itself. Four changes make it safe and three more make it pleasant (see
[What Ledger should do](#what-ledger-should-do)).

The recommended way is option A: the vault *is* the synced folder, and GitHub sync goes.

## How it works now

- `sync.rs` holds the three-way reconcile, behind a `Remote` trait. `github.rs` is the only implementation, and
  `encrypted_remote.rs` wraps any `Remote`. Each round compares against the base kept in `.ledger/sync-state.json`.
- A push only lands if the branch is still where this machine last saw it. GitHub refuses anything else
  (`Error::Rejected`), and the round starts over. That refusal is what stops two machines overwriting each other.
- Edits made outside Ledger are noticed when its window regains focus: the `focus` listener in `src/lib/ui.svelte.ts`
  calls `check_vault_changes`. There is no file watcher.
- `Vault::persist` writes every file whose rendering changed since Ledger last wrote it. It goes through `write_atomic`
  (a `.name.tmp` file renamed over the original) and doesn't look at the file on disk first.
- Per-machine state lives inside the vault, in `.ledger/`:
  - `.env`: the GitHub token, the encryption key, and the Trello API key and token, in plain text;
  - `settings.json`: the GitHub repository and branch, whether encryption is on, and the screenshot hotkey and tool
    (which differ per machine);
  - `sync-state.json`, `trello-sync.json` and `conflicts/`: sync bookkeeping.

  `.gitignore` keeps all of it off GitHub, but no sync client reads `.gitignore`.
- By default the vault lives in `Documents/Ledger` (`settings::resolve_vault`).

## What the providers do

| | OneDrive (personal) | OneDrive for work or school, SharePoint | Nextcloud |
|---|---|---|---|
| Both machines changed a file | keeps both, one renamed `name-COMPUTERNAME.ext` | the same | renames the local copy `name (conflicted copy YYYY-MM-DD HHMMSS).ext`, which by default isn't uploaded¹ |
| Earlier versions of a file | the last 25 | whatever the admin set: a count (often 500) or automatic limits | thinned with age on an automatic schedule; the admin can change it |
| Recycle bin | 30 days | 93 days, unless the admin changed it | 30 days ("auto"), sooner if the quota runs out |
| Undo everything since a date | Restore your OneDrive, up to 30 days back, for Microsoft 365 subscribers only | Restore your OneDrive; Restore this library on SharePoint | no; per file only (versions, trash). Server backups are the admin's job |
| Keeping a folder (`.ledger/`) out | not possible | only through an admin policy: Windows only, folder names without wildcards, reportedly not for SharePoint libraries | ignore patterns in the client; files whose names start with a dot aren't synced by default¹ |
| Files kept only online | Files On-Demand | the same | virtual files, when switched on |
| Names and paths | at most 400 characters per path and 255 per name; `" * : < > ? / \ \|` not allowed | the same | the server is case-sensitive and Windows and macOS aren't, which gives "case clash" errors |

¹ From third-party guides and forum posts: the conflicts page of the Nextcloud desktop docs currently returns 404. Check
on the client version in use.

Two more things:

- On Windows, a sync client that holds a file open makes renaming another file over it fail (EPERM or EBUSY). npm and
  Obsidian users run into this in OneDrive folders.
- Ledger autosaves, so a deliverable you're writing can use up OneDrive's 25 versions in an afternoon. Published versions
  (`name@v0.1.ext`) are separate files that never change, so the history that matters most doesn't depend on the
  provider.

## What goes wrong if the vault is just moved there today

**Settings → Vault** can already move the vault into a OneDrive or Nextcloud folder, and turning GitHub sync off then
leaves syncing to the provider. With one machine running Ledger at a time that mostly works. What goes wrong:

1. **`.ledger/` is uploaded** (OneDrive, SharePoint). The tokens and the encryption key end up in the cloud, and every
   machine reads the others' screenshot settings and sync bookkeeping. On Nextcloud it stays local, but only because
   dot-files are skipped by default.
2. **Changes from the other machine are silently overwritten.** They reach the disk while Ledger's window has focus, so
   Ledger doesn't see them, and the next edit to the same item writes over them. The provider's version history is
   then the only way back.
3. **Conflict copies get lost.** Items are read by the `id` in their frontmatter, and `INSERT OR IGNORE` drops a second
   file with the same id. Of `posts/<id>.md` and `posts/<id>-LAPTOP.md`, one is dropped without a word, and it can be
   the original. Its text stays in a file Ledger never shows. Under `deliverables/`, a copy shows up as an extra
   deliverable.
4. **Files kept only online** make the first start slow, because Ledger reads the whole vault, and make it fail offline.
5. **Writes now and then fail on Windows**, when the client holds the file being replaced. `persist` tries again with
   the next change, but the user sees an error.
6. **If GitHub sync is left on as well**, two sync engines work on the same files, and on OneDrive every machine shares
   one `sync-state.json`.

Point 1 already happens today on any machine where OneDrive backs up Documents (Known Folder Move, common on school and
work laptops). The default vault, `Documents/Ledger`, is then inside OneDrive, `.ledger/` included.

## The options

### A. The vault is the synced folder (recommended)

Ledger works on the folder the provider syncs, and GitHub sync goes.

- The provider syncs, keeps versions and restores. Ledger does none of that.
- The files stay readable in File Explorer, on onedrive.com and in Nextcloud's web pages, and Obsidian or any other
  tool can edit them on any machine.
- Code goes away (`github.rs` and most of `sync.rs` and `syncer.rs`), instead of a second sync engine running on top
  of the provider's.
- What it takes is the list under [What Ledger should do](#what-ledger-should-do).
- What it loses: the files in the cloud aren't encrypted. Where that matters (an organisation's SharePoint, say), put
  Cryptomator or something like it on top of the folder, or use option B.

### B. The synced folder as a remote

A `FolderRemote` next to `GithubRemote`. The vault stays where it is, and each round reconciles it with a copy in the
synced folder, as it does with GitHub now.

- It keeps the three-way merge and its base, `.ledger/` never leaves the machine, and encryption comes along unchanged,
  since `EncryptedRemote<R>` wraps any `Remote`.
- `Remote` has git's shape (`head`, `tree`, `blob`, `push`), and `push` relies on the remote refusing a stale push. A
  folder can't refuse anything. Two machines pushing at once still end up with the provider's conflict copies, and
  `FolderRemote` would have to recognise them.
- A store of files that never change would avoid that: content-addressed blobs, commits and one `HEAD` file, with lost
  races found and merged. But then the folder can't be read without Ledger, and the provider can only restore all of
  it or nothing.
- The vault takes twice the disk space, and a change takes two hops to reach another machine.

B is worth it only if encryption in the cloud is a must.

### Not considered: the providers' APIs

Talking to Microsoft Graph or Nextcloud's WebDAV directly would replace `github.rs` with one client per provider. That
goes against the point here: a folder that any provider can sync.

## What Ledger should do

To make it safe:

1. **Move per-machine state out of the vault.** Secrets go to the system keychain or the app's config folder
   (`keystore.rs` was written for that swap). Per-machine settings and sync bookkeeping go to the config folder, keyed
   by vault. This also fixes the Known Folder Move leak there is today.
2. **Watch the vault.** A file watcher (the `notify` crate) reloads, after a short pause, when the provider brings in
   changes, instead of waiting for focus.
3. **Look before writing.** The vault already keeps each file's size and modification time (`fingerprint`). `persist`
   can refuse to replace a file that changed since Ledger last saw it, reload, and keep both versions for the user to
   choose between.
4. **Recognise conflict copies, whatever the provider.** Every item's file is named after its id, so a file whose name
   isn't its frontmatter `id` is a copy. For `thread.md`, `project.md`, `README.md` and deliverables, match the
   providers' patterns: OneDrive's `-COMPUTERNAME`, Nextcloud's `(conflicted copy …)`, and Dropbox's and Syncthing's if
   those are to be supported. Keep copies out of the database and list them in a Conflicts view, with a diff, *keep
   mine* and *keep theirs*.

To make it pleasant:

5. **Know the provider.** Windows lists sync roots under `HKCU\Software\SyncEngines\Providers`, macOS keeps them under
   `~/Library/CloudStorage`, and a Nextcloud folder holds the client's sync database. The dashboard's GitHub card
   becomes, for example, "Backed up by OneDrive: last 25 versions, recycle bin 30 days, restore on onedrive.com", and
   it warns when the vault isn't in a synced folder at all.
6. **Keep the vault on the device.** On Windows, pin it through the Cloud Files API (`CfSetPinState`; `attrib +P` does
   the same by hand). That should cover Nextcloud's virtual files as well as OneDrive, but this isn't confirmed. On
   macOS, ask the user to choose "Always keep downloaded".
7. **Try a failed rename again** a moment later, when Windows says the file is in use.

Moving over means two things for an existing vault: Settings → Vault moves it into the synced folder (the existing
"copy the current vault into it"), and GitHub sync is switched off.

## Before building

- Moving `.ledger/` out of the vault and retiring GitHub sync change how existing vaults work, so both are breaking
  changes (`feat(sync)!`, with a footer saying what to do). They belong on `integration/1.0.0`.
- Check on real clients: Nextcloud's conflict name and whether it really skips dot-files, what OneDrive on macOS does
  with `.ledger/`, and whether `CfSetPinState` works on Nextcloud's virtual files.
- Decide whether encryption in the cloud is needed. If not, option A. If it is, option B or Cryptomator.

## Sources

- Microsoft: [Restore a previous version of a file](https://support.microsoft.com/office/159cad6d-d76e-4981-88ef-de6e96c93893),
  [Restore your OneDrive](https://support.microsoft.com/en-us/onedrive/restore-your-onedrive),
  [Restore deleted files](https://support.microsoft.com/en-US/onedrive/restore-your-onedrive-files),
  [Version history limits](https://learn.microsoft.com/en-us/SharePoint/document-library-version-history-limits),
  [Restrictions and limitations](https://support.microsoft.com/help/64883a5d-228e-48f5-b3d2-eb39e07630fa),
  [CF_PIN_STATE](https://learn.microsoft.com/windows/win32/api/cfapi/ne-cfapi-cf_pin_state)
- OneDrive exclusions: [Excluding specific folders](https://blog-en.topedia.com/2026/09/excluding-specific-folders-from-onedrive-sync-on-windows-clients/),
  [Excluding specific files](https://petervanderwoude.nl/post/excluding-specific-files-from-being-uploaded-to-onedrive/)
- OneDrive conflicts: [UWindsor KB](https://uwindsor.teamdynamix.com/TDClient/1975/Portal/KB/Article/29832/Known-Issue-OneDrive-duplicate-files),
  [SharePoint Maven](https://sharepointmaven.com/how-onedrive-sync-resolves-sync-conflicts/)
- Nextcloud: [Versions](https://docs.nextcloud.com/server/stable/admin_manual/configuration_files/file_versioning.html),
  [Trash bin](https://docs.nextcloud.com/server/stable/admin%5Fmanual/configuration_files/trashbin_configuration.html),
  [Desktop client](https://docs.nextcloud.com/server/stable/user_manual/en/desktop/usage.html),
  [Conflict copies stay local](https://help.nextcloud.com/t/nextcloud-conflict-server-version-downloaded-local-copy-renamed-and-not-uploaded/53338),
  [Hidden files (third party)](https://awesome.ecosyste.ms/projects/github.com%2Fwarp-lab%2Fnextcloud-settings-for-programmers),
  [Conflict naming (OpenCloud, same lineage)](https://docs.opencloud.eu/docs/user/desktop-client/file-conflicts)
- Others in synced folders: [Obsidian, OneDrive and EBUSY](https://forum.obsidian.md/t/onedrive-and-ebusy/85929),
  [Obsidian, OneDrive duplicates](https://forum.obsidian.md/t/vault-synced-over-onedrive-across-multiple-machines-generates-many-copies-of-workspace-json/115229),
  [npm EPERM in OneDrive](https://github.com/npm/npm/issues/19956)
