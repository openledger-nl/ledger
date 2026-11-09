# Ledger

A local desktop app for thinking in **threads**, asking for **deliverables** and giving feedback on them, and planning the work on a **board** — all inside **projects**. Tauri 2 (Rust) on the back, SvelteKit (Svelte 5) on the front. Your data is a **vault** — a folder of markdown files with YAML frontmatter, like an Obsidian vault — that can be mirrored to a private GitHub repository.

This is the rewrite of the original ASP.NET ledger, which is kept in the author's private repository (MilanMolenbrugge/ledger, commit `3ec6769`). The data model carries over in spirit, and so does the idea of plain files synced to GitHub; the shell does not. The look follows the Leerjournaal app: a dark sidebar, calm cards, one accent colour, six themes.

## How it keeps your data

```
   the vault (files)  ──load at start──▶  in-memory database  ◀── every read
         ▲                                       │
         └────────── every change, written through ┘
         │
         └── GitHub (private repo): pulled at start, pushed after changes, checked every few minutes
```

- **The vault is the source of truth.** Open it in Obsidian, grep it, put it in git yourself. Edits made outside Ledger (in Obsidian, by margin-code in VS Code, by a `git pull`) are noticed while it runs: the vault folder is watched, and a second after the last change the files are read in again, the screens refresh, and the change goes to GitHub like any other. Ledger's own writes, its `.ledger/` folder, dotfiles and attachments don't count. If the system can't watch the folder, edits are picked up when the window regains focus, which is checked as well.
- **At startup** the whole vault is read into an in-memory database. Screens are answered from memory; every change is written straight back to the files (only the files that actually changed, and atomically).
- **GitHub is the store you sync to.** Ledger talks to GitHub's REST API directly, so nothing else needs installing. Each round is a three-way comparison per file against what both sides last agreed on, so changes made on two machines merge file by file. Binary files (uploads, images) sync too. If the same file was edited on both sides, this machine's version wins and the other is kept in `.ledger/conflicts/`; an edit always beats a delete. Ledger refuses to sync to a public repository.

### The vault

```
projects/<project>/project.md                       a project (name, board columns)
projects/<project>/README.md                        the project's reading guide — where to start
projects/<project>/threads/<thread>/thread.md       a thread
projects/<project>/threads/<thread>/posts/<id>.md       its posts
projects/<project>/threads/<thread>/prompts/<id>.md     its prompts: requests for deliverables
projects/<project>/threads/<thread>/feedback/<id>.md    its feedback, which lands on a deliverable
projects/<project>/threads/<thread>/uploads/<id>.md     upload manifests: when, where the file is, a description
projects/<project>/threads/<thread>/runs/<id>.md        a run of a prompt with Claude Code: how it ended, what it did
projects/<project>/threads/<thread>/reviews/<id>.md     a code review written with margin-code (Ledger only reads it)
projects/<project>/threads/<thread>/reviews/<id>/<id>.md   its comments and their replies
projects/<project>/userstories/<id>.md              a user story
projects/<project>/tasks/<id>.md                    a task (a card on the board)
projects/<project>/deliverables/                    what prompts produce — any kind of file; only the live files
projects/<project>/versions/                        the published versions, tagged name@v0.1.ext, each at the same place
                                                    under versions/ as its live file is under deliverables/
projects/<project>/versions/<id>.md                 how a published version came to be: who made it, the thread, prompt
                                                    and run behind it, its AI-use label
projects/<project>/attachments/                     files uploaded to a thread, screenshots included
projects/<project>/templates/<id>.md                templates for this project
templates/<id>.md                                   templates for every project
semesters/<id>.md                                   a semester (half a year) and its sprints
.ledger/                                            settings, the secret and sync bookkeeping — never synced
.gitignore                                          lists .ledger/ and .env (so the vault can also be a git repo)
```

Files Ledger doesn't know about are left alone, so the vault can hold other notes. Hand-written files are read leniently: a folder of posts without a `thread.md` is a thread, a post without frontmatter is all body, links to things that don't exist are dropped, and the repairs are written back. Files dropped into `deliverables/` by hand (or by an assistant) show up in the app.

### Choosing where the vault lives, and syncing it

**Settings → Vault** shows the folder and lets you pick another (optionally copying the current vault into it when it's empty). The location itself is the one thing kept outside the vault, in the app's config folder.

**Settings → Sync to GitHub** takes a repository (`owner/name` or a github.com URL), a branch (created on the first sync if it doesn't exist) and a repository-scoped access token. The repository and branch go in `.ledger/settings.json`, next to the other per-vault settings (the screenshot hotkey); the token goes in `.ledger/.env`:

```
GITHUB_TOKEN=github_pat_…
```

`.ledger/` is gitignored — and never uploaded even if the `.gitignore` is edited — so neither file leaves this machine. The app reads them at startup to see whether sync is set up; if everything is filled in it pulls straight away. The token never reaches the UI (only that one is saved, and its last four characters).

Use a [fine-grained token](https://github.com/settings/personal-access-tokens/new) limited to that one repository with **Contents: Read and write**. The repository must already have a commit (a README will do).

### Encrypting what goes to GitHub

**Settings → Sync to GitHub → Encryption** seals the vault before it is uploaded. The vault on disk stays plain markdown; only the copy on GitHub is encrypted, with a key that stays on this machine (`LEDGER_ENCRYPTION_KEY` in `.ledger/.env`) and is never uploaded. Export the recovery key to use the same repository on another machine, and keep a copy somewhere else: without the key, the copy on GitHub can't be read by anyone.

- **What is encrypted**: the contents of every file (compressed, then AES-256-GCM) and every file's path. On GitHub each file is stored under a keyed hash of its path (32 hex digits, all side by side in the top folder), and one more file, `manifest`, holds the real paths, encrypted. No project, thread, deliverable or upload name can be read there, and neither can how the files are grouped into folders. A file's encryption is bound to its real path, so a file put under another name on GitHub doesn't open.
- **What stays visible** to anyone who can read the repository: that it is a Ledger vault, how many files it holds, roughly how big each one is (about its compressed size), and when files are added, changed or removed: the commits, whose messages count the files updated and removed. The manifest changes when a file is added or removed, not when one is edited.
- **Encryption starts fresh**, on a new repository or on a branch that doesn't exist yet. A branch that already holds the vault in plain text is never converted, because its history would keep the files. Nor is a branch encrypted by an earlier version of Ledger, which left the paths readable: sync stops there and says so. Change the branch to a new one, on every machine, and the vault goes up there with its names hidden; the old branch can be deleted on GitHub once every machine has moved.

## What it does

- **Getting started** — a client with a vault that has nothing in it yet opens on the home page, where a guide offers two ways in. **Import an existing vault** from a folder on the computer, or from the GitHub repository it is synced to (with the key first, when that is encrypted). Or **set up a new one** in four steps that tick themselves off as they are done: a semester (the NJ26 preset or your own), sync to GitHub (with encryption, if you want it, chosen before the first sync), a first thread (with its project, named on the spot), and a first deliverable, which ends the guide. The semester and the sync can be put off. A vault that already has a project or a semester never gets the guide, and **Settings → Sync & external → Vault** brings it back.
- **Projects** — each has a reading guide (its `README.md`, rendered and editable on its Overview tab, under the project's latest threads and deliverables with buttons for a new thread and a new deliverable), threads, deliverables, user stories, a board and templates, each on a tab of its own. In the explorer on the left a project unfolds into its threads and deliverables, with a button for a new thread and a new deliverable; opening another project folds the first, and clicking the unfolded one folds it without leaving the page you are on. Above the projects, a Latest section lists the newest threads and deliverables across all of them. Deleting a project removes its folder.
- **Threads** — one per topic you're juggling. A timeline of markdown posts (replies quote their parent; edit and delete), prompts, feedback, uploads and code reviews written with margin-code (see below), grouped by day. Threads can be linked to each other, in both directions, and to user stories.
- **Prompts** — a request for a deliverable, written in a thread, in one dialog wherever it is made (a thread, a deliverable's Prompts window, feedback you have selected). It starts with the **deliverable**: an existing one (the dialog opens on the one you are editing), which offers its open feedback to resolve, one piece or **all of it**; or a new one, which can be given a file name (blank: whatever produces the prompt names the file) and started from a **template**, whose guiding questions are added to the thread as posts to answer first (a one-time snapshot). What a prompt says is two texts: a **base prompt**, filled in from the template's structure or from the feedback (each piece with the version it was given on) and folded away until you want to edit it, and **what you want**, in your own words. A prompt has no title to write: Claude Code names it from what it asks, once, in the background (a small model, no tools, nothing of the vault in reach; Settings → Claude Code turns it off), and until then it is called by the start of its text. A prompt belongs to a **deliverable**, not to one of its versions: the deliverables it is about are changed under Edit, and the versions it produces know the prompt and the run in their own records. A run with Claude Code is told which versions the deliverable has, as files to read and never to change.
- **Deliverables** — files in the project's `deliverables/`. Markdown ones are edited in the app, autosaved, in source mode (the raw markdown), live preview or reading mode, as in Obsidian; any other kind opens with the system's program. **Publish** a version (minor, major or a custom `M.m`) to freeze an immutable copy named `name@v0.1.ext` in the project's `versions/`, which also drops a note in the thread of the prompt that produced it. Keeping the versions there leaves `deliverables/` with only the live files, so it reads cleanly in Obsidian or a file browser; a vault from before 1.0 has its versions moved there when Ledger opens it. Each deliverable has a side window on its right, with a button on the rail beside it for **Produced by** (see below), for **Feedback** (what you wrote on the file, by thread and under each by version, and a box to add more) and for **Prompts** (the ones about this file: make one, edit it, run it, add feedback to it): one is open at a time, and its button or the X closes it. The versions are switched in that window: click **Live** or a version's chip there. A **⋯** menu next to Publish holds Copy markdown, Open in editor, Show in folder and Delete deliverable. Open feedback can be ticked, several pieces at once, and made into one prompt or added to a prompt that is there. Everything done on the page goes to one **thread**, chosen in the box at the top and remembered for that deliverable: the note a publish leaves, new feedback, and the prompts made from feedback. Feedback is on the whole document, or on a **passage**: select text in any mode (source, live preview or reading) and click the comment icon after the selection. The passage is kept as plain text with a few words around it (`quote`, `quotePrefix` and `quoteSuffix` in the feedback's file), so it is lit up in the document wherever it is found again, a click on it shows its feedback, and the feedback says so when the text it was about has changed.
- **Produced by, and AI use** — every published version records who made the change: you (you typed it), Claude Code (a run started from the app, which also links the version to its prompt), or AI you say you used elsewhere. The **Produced by** window on the right of a deliverable opens with a summary, then lists the live file as the newest version (a draft the moment it holds more than the last one, as you type), then its threads and under each thread what happened there, in the order set under Settings → Order (newest first unless you change it): the prompt that asked for the file, the run, each version with who made it, its size and a SHA-256, and the feedback on it. The "version published" note in a thread links to the prompt that caused the version, and quotes the changelog you wrote for it ("from changelog: …"), if you wrote one. When you publish, a checkbox says the change was made with generative AI (it starts ticked when a Claude Code run changed the file and nothing published it), and you can attach a label from the [Me & My Machine](https://mmmlabel.tech) scheme — Lazy Prompter, Piggybacker, Cyborg, Handyman or Man-Made. Ledger records the facts; the label is yours to declare, per version, because it describes the product as it stands then. Versions published before this existed show what the runs and notes prove (versions a run published are known to be AI) and otherwise "origin not recorded". On any version, click who made it (**You** or **AI**) or its label to change it; a version a Claude Code run made keeps its author. The records are plain files, so they can be edited by hand.
- **Feedback** — written in a thread, landing on a deliverable or on one of its published versions; open or resolved.
- **Uploads** — pick files in a thread (they are copied into the project's `attachments/` and noted in an `uploads/<id>.md` manifest with a description). Screenshots land the same way. A project's **Attachments** tab lists every file uploaded in it, from all its threads (in the order set under Settings → Order), with a thumbnail for pictures: filter them by name, description or thread, or to images or other files; open one, show it in its folder, or go to the thread it was uploaded in.
- **Templates** — a name, a "use when…" hint, guiding questions and the skeleton of the deliverable. Global ones (Settings → Templates) serve every project; a project can *customise* one, which copies it into the project, where it replaces the shared one.
- **User stories** — an id you choose (`MM01`), a summary, "As a / I want to / Such that", notes and acceptance criteria you tick off. A story links to any number of threads.
- **Board** — per project, with customisable columns (Backlog → Doing → Done by default). Tasks can point at a story and a thread. Drag a card, or use its arrows, to change column; a column that still holds tasks can't be deleted.
- **Screenshots** — press a global hotkey (default Ctrl/⌘+Shift+S) anywhere: your own screenshot tool (Flameshot, say) or the built-in capture takes the picture, and Ledger offers to attach it to the thread you have open. Pictures sent to Ledger with "Open with" land the same way. See below.
- **Writing** — every markdown field (notes, prompts, feedback, descriptions, deliverables, templates, the reading guide) is the same inline editor ([CodeMirror 6](https://codemirror.net)). It edits the markdown itself, so the file gets exactly what you typed, but reads like the rendered page: headings, bold, code and links are shown as such and their marks fade away except where the cursor is, tables are drawn as tables until you click into one, and pictures are shown in place. Task boxes can be ticked, Ctrl/⌘+click opens a link, and Ctrl/⌘+B, I and K make bold, italic and a link. **Links between things** are written as in Obsidian, in every one of those fields: `[[` offers the project's threads, deliverables and feedback (type to narrow the list, Enter picks), and the brackets close themselves. `[[Pricing model]]` is a thread, by its title; `[[pricing-brief]]` a deliverable, by its file name (`[[plans/pricing-brief.md]]` by its path, and a file that isn't markdown keeps its extension); `[[feedback:<id>|too long]]` a piece of feedback, which has no name, so the link carries words of its own (the list writes them from what the feedback says). `[[target|words]]` shows other words; `[[thread:Name]]` and `[[deliverable:name]]` spell out the kind when a thread and a deliverable have the same name. In live preview a link reads as its words, and its brackets show where the cursor is; Ctrl/⌘+click follows it, and on the rendered page a click does. A link that points at nothing is faint and dashed. A link finds things in its own project, by what they are called when it is followed: renaming a thread or a deliverable does not rewrite links to it. The links stay as typed in the files; Obsidian reads them as its own links, which open there only when a note of that name exists. In a vault-wide template there is no project to point into, so nothing is offered there.
- **Dashboard** (Home) — a card per project, the most recently active first, with its three latest threads and two latest deliverables, and links to its board, a new thread and a new deliverable. **Pin** a project (the pin on its card) to keep it at the top. Above them, a contribution graph like GitHub's: a square per day, shaded by how much you made that day, for all projects or one. It opens on the semester you're in (or the nearest one; with none set up it says so and links to Settings → Semesters), every day of it drawn: today has a ring, the days that haven't happened yet are dashed and empty, and the stats count the days left. Or pick a year you made something in. It counts what you make: threads, your posts, prompts, feedback, uploads, published versions, Claude Code runs, stories and tasks; not edits, which leave no history, nor what the app writes for you. Switch it off under Settings → Dashboard. Pins and that setting are kept per machine, like the theme. Below the projects a **GitHub sync** card shows GitHub as the backend: the repository and branch, when it last synced and what that did, how many files GitHub holds of the vault (notes, deliverables, attachments: counted from the sync's own bookkeeping, so it costs no request), the commit both sides agree on and how many commits the branch holds (the one thing it asks GitHub, after each sync), whether changes are waiting and whether it is encrypted, with Sync now and Settings buttons (Settings → Sync to GitHub shows the same card above its form); without sync it offers to set it up. The explorer's footer says the same in three lines: how it stands and when, `owner/repo@branch`, and the encryption.
- **Semesters** (Settings → Semesters, or the Semesters button on the dashboard's contribution graph) — half years you set by hand, listed there, with a + to add one: the **NJ26** preset (autumn 2026: a kick-off week, a shorter sprint 0, five sprints of three weeks, a week off in autumn and two at Christmas; named "NJ26 - Semester X" until you fill in the X) or a semester of your own: a name, a first and a last day, and the vacations in them. Each is cut into sprints of three weeks of work (or another length for all, and any sprint can have a length of its own) from its first day. The sprint clock stops for a vacation: a sprint that runs into one carries on after it, one that would start in it starts after it, and less than a week of work left at the end joins the last sprint. So the sprints always fit; you name them and give them goals. Semesters can't share days, so a day belongs to one semester at most. Kept as `semesters/<id>.md`, synced like everything else; on the dashboard's graph vacation days are outlined.
- **Dates** are shown the European way everywhere: day before month ("3 Nov 2026", typed as 03-11-2026) and a 24-hour clock. Files keep the ISO form (2026-11-03) that YAML and other tools expect; a hand-written semester may use either.
- **Search** across every post, prompt, feedback and deliverable, with highlighted snippets. **Themes**: Sepia (the default), Dark, Night, Forest, Ocean, Light. **Order** (Settings): one choice, newest first by default, for the posts of a thread, the threads in the sidebar, the feedback, versions and "Produced by" window of a deliverable, and a project's attachments. It only changes how things are shown. **Settings → Features** also turns the contribution graph and the explorer's **Latest** section (the newest threads and deliverables, under the projects) on and off. **Name prompts** (Settings → Claude Code, on by default): a new prompt is named by Claude Code from what it asks. **Run prompts by themselves** (Settings → Claude Code, off by default): a prompt you create starts running in Claude Code at once, without the confirmation of what will be sent.

## Code reviews with margin-code

[margin-code](https://github.com/openledger-nl/margin) (the `code/` folder there) is a VS Code extension that reviews a diff of any local git repository, whatever forge it lives on, and keeps every review in the Ledger vault, in a thread, as markdown files. So a review in progress reaches your other machines through Ledger's own sync, and a finished one stays with the thread it belongs to. Posting reviews to GitHub or GitLab is for margin-code, later.

Ledger shows each review in its thread's timeline, on the day it was started: in progress or its verdict, the repository, what was compared (`main..feat/code`, with short SHAs), the summary, and the comments by file, each with its place (`path:line`), the code it is about, its replies and whether it is resolved. The cards are read-only: reviews are written and edited in VS Code.

Ledger reads the files leniently, like everything in the vault, and never writes them: not when it writes its own files through, not on a reload. They go only with their thread or project, when you delete that in Ledger. A file in a newer format than Ledger knows is shown as far as it can be read, with a note. The format is shared with margin-code and fixed: change it only in step with margin-code.

### The contract: margin review format, version 1

Paths are relative to the vault root. Ids are Ledger's: `<epoch-millis>-<8 chars of [0-9a-z]>`, which sort chronologically as strings. The file name is the id plus `.md`. Timestamps are ISO-8601 UTC with milliseconds (`2026-11-09T10:15:00.123Z`). Frontmatter is YAML with camelCase keys. The body is markdown.

```
projects/<project>/threads/<thread>/thread.md                       the thread (Ledger's own format)
projects/<project>/threads/<thread>/reviews/<review>.md             a review: one round of reviewing one diff
projects/<project>/threads/<thread>/reviews/<review>/<comment>.md   a comment in it, or a reply
```

Review, `reviews/<review>.md`:
```yaml
---
format: 1
repo: github.com/openledger-nl/margin   # repo identity, see below
repoName: margin                        # for display
base: <40-char sha>                     # the commit compared against
baseRef: main                           # optional, display only
head: <40-char sha>                     # the commit reviewed
headRef: feat/code                      # optional, display only
state: draft                            # draft | submitted; submitted is final, a next round is a new review
verdict: approve                        # approve | changes | comment; only when submitted
author: Milan Molenbrugge               # git user.name
createdAt: 2026-11-09T10:15:00.123Z
updatedAt: 2026-11-09T10:15:00.123Z
submittedAt: 2026-11-09T11:00:00.000Z   # only when submitted
---
The summary in markdown (may be empty while a draft).
```

Comment, `reviews/<review>/<comment>.md`:
```yaml
---
format: 1
path: browser/server/src/mcp.ts   # repo-relative, forward slashes, the file as it is on `side`
side: new                         # new = the head's version, old = the base's (e.g. a removed line)
line: 42                          # 1-based first line; leave line and endLine out for a comment on the whole file
endLine: 44                       # last line; equal to line for a single line
commit: <40-char sha>             # the commit those line numbers belong to
code: |                           # the commented lines, verbatim: readable without the repo, and how the place is found again after the code moves
  …
codeBefore: |                     # optional, up to 3 lines before
  …
codeAfter: |                      # optional, up to 3 lines after
  …
replyTo: <comment id>             # only on a reply; a reply leaves out path/side/line/endLine/commit/code and sits where its parent does
resolved: false                   # on the first comment of a conversation
author: Milan Molenbrugge
createdAt: …
updatedAt: …
---
The comment in markdown.
```

**Repo identity.** Take the `origin` remote, or else the first remote by name, as host/path. Lowercase the host, and drop the scheme, user, port and trailing `.git`. So `git@github.com:openledger-nl/margin.git` and `https://github.com/openledger-nl/margin` both become `github.com/openledger-nl/margin`. A repo without remotes is `git:<sha of its root commit>`.

**Who writes what**
- margin-code writes and deletes review and comment files. Ledger reads them and never rewrites or deletes them, except together with their thread or project when the user deletes that in Ledger.
- margin-code may create a thread for a review. It makes a new folder `threads/<slug>/` with a `thread.md` holding `title` and `createdAt` (the body is an optional description), the slug made as `ids::slugify` makes it, with `-2`, `-3`… when taken. After creating it, margin-code never writes that `thread.md` again: it is Ledger's, and Ledger may rewrite it canonically.
- Writes are atomic: a temp file whose name starts with `.` in the same folder, renamed over the target. Ledger ignores dotfiles.
- Both sides keep unknown frontmatter keys as they are and otherwise ignore them. A file with a `format` higher than a side knows is shown read-only by Ledger and left untouched by margin-code, with a note.
- While Ledger runs, it notices these files within seconds (the watcher from step 1) and syncs them. Until it runs, they wait on disk.

## Screenshots

**Settings → Screenshots** holds the hotkey and the tool. Both are stored per vault in `.ledger/settings.json`, so they are never synced, and each machine can use its own tool. The hotkey is set with a key recorder, checked (it needs a modifier, and the system's own shortcuts are refused) and registered immediately, replacing the old one without a restart; if the system won't register it (usually because another program owns it) you get a warning and the old shortcut keeps working.

When a picture arrives, Ledger holds the PNG in memory, emits `screenshot-ready`, brings its main window forward and asks where it goes. With a thread open that is a short confirmation (that thread, an optional description, Enter); *Change…* or no open thread gives the full choice of project and thread (or a new one). *Attach* saves the PNG in the project's `attachments/`, writes the upload manifest and opens the thread at it; *Discard* drops it. Failures are shown the same way.

### Your own tool (bring your own)

Choose **My own tool** and give a command. Ledger runs it when the hotkey is pressed; it is split into a program and its arguments by Ledger itself (spaces separate, quotes group, backslashes are kept), so it means the same everywhere and needs no shell.

- With `{file}` in it, Ledger fills in a fresh temporary `.png` path, waits for the tool to exit and reads the file.
- Without `{file}`, Ledger reads the PNG the tool prints.
- No picture means cancelled, unless the tool printed an error (anything but an "aborted"/"cancelled" message), which is shown.

Presets, offered in Settings with whether the program was found:

| | Command |
|---|---|
| Linux, Flameshot | `flameshot gui --raw` |
| macOS, built-in picker | `screencapture -i {file}` |
| macOS, Flameshot | `/Applications/flameshot.app/Contents/MacOS/flameshot gui -p {file}` |
| Windows, Flameshot | `"C:\Program Files\Flameshot\bin\flameshot.exe" gui -p {file}` |

### Opened with Ledger

A PNG passed to Ledger on the command line becomes the screenshot waiting to be attached; a second launch is handed to the running app by [tauri-plugin-single-instance](https://crates.io/crates/tauri-plugin-single-instance), and on macOS opened files arrive as `RunEvent::Opened`. That is what Flameshot's *Open with* (Ctrl+O in its editor) uses.

- **Linux**: the .deb and .rpm install a desktop entry with `Exec=ledger %F` and `MimeType=image/png` (`src-tauri/linux/ledger.desktop`, set in `tauri.linux.conf.json`), so Ledger is listed. With `tauri dev` there is no desktop entry; to try it, put one in `~/.local/share/applications` whose `Exec` is `src-tauri/target/debug/ledger %F`.
- **macOS**: `tauri.macos.conf.json` declares PNG with rank *Alternate*: Ledger appears under Finder's *Open With* but never becomes the default viewer.
- **Windows**: no file association, because Tauri's installer would make Ledger the default PNG program. Pick `Ledger.exe` through *Open with → Look for another app on this PC*.

### The built-in capture

Used when no tool is chosen. It is meant to be removable (the list of what to delete is at `builtin` in `src-tauri/src/screenshot.rs`).

- **Windows, macOS**: the monitor under the cursor is captured ([xcap](https://crates.io/crates/xcap)); a frameless, always-on-top window shows it so you can drag out a region (Esc or a right-click cancels); the backend crops it, scaling the drag from window units to capture pixels so HiDPI screens come out exact. macOS needs the Screen Recording permission: Ledger checks it before capturing, triggers the system prompt the first time, and otherwise says where to switch it on (Ledger must be restarted afterwards). The overlay is opaque on macOS (a transparent window needs Tauri's `macos-private-api`).
- **Linux**: the desktop's screenshot portal ([ashpd](https://crates.io/crates/ashpd)), with the compositor's own area picker. xcap isn't built on Linux: it pulls in PipeWire, which needs `libpipewire-0.3-dev` and libclang just to compile.

Under Wayland the hotkey is bound through the global-shortcuts portal; if the desktop doesn't offer it, Settings says so and the *Take a screenshot now* button still works.

The built-in capture on Windows is exercised end to end. The macOS paths, and the Flameshot presets on Windows and macOS, have been type-checked but not run.

## Running it

Install a release to use Ledger and get updates from inside the app; build it from source to work on it.

### Installing a release

Every [release](https://github.com/openledger-nl/ledger/releases) carries installers:

| System | Download |
|---|---|
| Windows (64-bit) | `Ledger_X.Y.Z_x64-setup.exe`: installs for your account only, no administrator needed |
| macOS (Apple silicon and Intel) | `Ledger_X.Y.Z_universal.dmg`: open it and drag Ledger to Applications |
| Linux (x86-64) | `Ledger_X.Y.Z_amd64.AppImage` (`chmod +x` it and run it), or the `.deb` / `.rpm` for your package manager |

The installers aren't signed with a Windows or Apple developer certificate yet, so the first start needs one extra step:

- **Windows**: SmartScreen says "Windows protected your PC". Click **More info**, then **Run anyway**. Your browser may also ask whether to keep the download.
- **macOS**: the first time you open Ledger, macOS says it can't check it for malicious software and won't open it. Click **Done**, open **System Settings → Privacy & Security**, find "Ledger was blocked to protect your Mac" and click **Open Anyway**, then confirm. (Before macOS 15, right-clicking Ledger in Applications and choosing **Open** also works.) Without a developer certificate, macOS may ask again for the Screen Recording permission after an update.
- **Linux**: nothing extra. The `.deb` and `.rpm` also add the desktop entry that *Open with* uses (see [Screenshots](#screenshots)).

Updates don't need these steps again: Ledger downloads them itself and checks their signature.

### Updates

Ledger looks for a new release shortly after it starts and every few hours. When one is out, a notice in the corner shows the version and what changed, with **Install and restart** and **Later**. A major version (the first number goes up) and its breaking changes stand out: something you rely on may work differently afterwards. **Later** hides the notice until the next check. Nothing is installed until you click **Install and restart**.

Installing downloads the new version and checks that it is signed with Ledger's key (the public half is built into the app, `plugins.updater` in `src-tauri/tauri.conf.json`); an update with a wrong or missing signature is refused, and the notice says so. It then waits until a restart cuts nothing off: a GitHub or Trello sync that is running finishes first, and if Claude Code is still running a prompt, you choose between waiting for it and stopping it. Then Ledger installs and restarts. Every change you make is written to the vault straight away, and changes not yet on GitHub are pushed when Ledger starts again. An update replaces the program only: the vault, its `.ledger/` folder and Ledger's config are left alone.

**Settings → About** shows the version you have and has **Check for updates**. To stop the automatic checks, untick **Check for updates automatically** there; **Check for updates** still works. The choice belongs to this computer, not to the vault: it is kept in Ledger's `config.json` in the app's config folder (next to the vault's location), so another computer syncing the same vault keeps its own.

Ledger built from source with `pnpm run dev:app` (scripts/dev.mjs) runs from your checkout as `pnpm run tauri dev` does, but nothing that changes there is put in front of you until you say so. It keeps watching the files: a change to the front end doesn't reload the window, and a change to the Rust side is built in the background while the app keeps running the build it started with. Ledger then shows what is waiting: the commits the checkout's branch gained since the running build (merged by you, or by an assistant working in another worktree; read from the local branch, nothing is fetched), and whether the new build is still being built, ready, or failing. Only **Update** brings it in, once a sync or a Claude Code run has finished: the window reloads when only the front end changed, otherwise Ledger restarts into the new build (and installs the packages first when `package.json` changed). **Not now** keeps the prompt away until something else changes. Quitting Ledger ends `pnpm run dev:app`; starting it again runs whatever the checkout has. Plain `pnpm run tauri dev` still reloads and restarts by itself as files change, and says nothing; a debug build that isn't served by the dev server (`tauri build --debug`) never looks. A release build you make yourself looks for releases, and offers the official ones.

### Building from source

You need [Node.js](https://nodejs.org) 26 (the version in `.nvmrc`; `nvm use` picks it) with [pnpm](https://pnpm.io) (`corepack enable` activates the version pinned in `package.json`), a Rust toolchain and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform (on Windows: the MSVC build tools and WebView2).

```bash
pnpm install
pnpm run tauri dev     # the app, with hot reload
pnpm run dev:app       # the app, with every change held until you choose Update in it
pnpm run tauri build   # an installer / bundle for this machine
```

A local `tauri build` makes the installers but not the updater's signatures: those need the private key, which only CI has.

Checks:

```bash
pnpm test              # Rust: stores, vault round-trips, and the sync algorithm against a fake GitHub
pnpm run check         # Svelte + TypeScript
```

Environment variables, mostly for development: `LEDGER_VAULT` fixes the vault location (and locks it in Settings); `LEDGER_CONFIG_DIR` moves the per-machine settings (the vault location, whether to check for updates); `LEDGER_GITHUB_API` points the sync at another API base (GitHub Enterprise, or a mock).

To send a change, read [CONTRIBUTING.md](CONTRIBUTING.md): the checks, and how to write the commits.

## Versions and releases

Every push to `main` that brings [conventional commits](https://www.conventionalcommits.org) is released by CI
([.github/workflows/release.yml](.github/workflows/release.yml)). The commits since the last release decide the bump,
the largest one wins:

| Commit | Bump | Example |
|---|---|---|
| a breaking change: `feat!:`, `fix(sync)!:`, or a `BREAKING CHANGE:` footer | major | 0.4.2 → 1.0.0 |
| `feat:` | minor | 0.4.2 → 0.5.0 |
| any other type: `fix`, `perf`, `refactor`, `docs`, `style`, `test`, `build`, `ci`, `chore`, `revert` | patch | 0.4.2 → 0.4.3 |

Changes reach `main` as pull requests merged with a merge commit, so every commit on a branch lands as it is and
counts (merge commits don't); [CONTRIBUTING.md](CONTRIBUTING.md) lists the types and scopes a commit may use, and a
check holds every commit in a pull request to them.

Commits that aren't conventional are left out; with none that count, there is no release. The version is written to
`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `Cargo.lock` (the app shows the Cargo one), the
notes go to [CHANGELOG.md](CHANGELOG.md), and CI commits that as `chore(release): vX.Y.Z`, tags it `vX.Y.Z` and
creates the GitHub release as a draft. It then builds the installers from the tag on Windows, macOS and Linux, signs
them for the updater with the password-less key in the repository secret `TAURI_SIGNING_PRIVATE_KEY`, uploads them with
their `.sig` files and the `latest.json` the app reads, and publishes the release once every platform has been built. A
release whose build failed stays a draft, which the app never sees. Don't bump versions by hand.

`main` only takes pull requests (the `protected-main` ruleset), so CI pushes that release commit and tag with a deploy
key the ruleset lets through: the write key whose private half is the Actions secret `RELEASE_DEPLOY_KEY`.

```bash
pnpm release:preview   # what the next release would be, and its notes; changes nothing
```

## Layout

```
src/                     SvelteKit frontend (static SPA)
  routes/                / · projects/[project] (overview) with threads/[thread], deliverables(/view),
                         stories, board, templates · search · settings (with Semesters and Templates)
  lib/api.ts             typed wrappers over the Tauri commands
  lib/ui.svelte.ts       toasts, dialogs, theme, and the backend's sync/vault/screenshot events
  lib/updates.svelte.ts  the update notice's state, and installing (waits, then restarts)
  lib/markdownEditor.ts  the markdown editor (CodeMirror 6): looks, live preview of the marks, tables, source mode, keys, [[links]]
  lib/links.ts           [[links]]: which thread, deliverable or feedback a typed target means, and what to offer after `[[`
  lib/links.svelte.ts    the open project's links, and where following one leads
  lib/vaultImages.ts     pictures in markdown: read from the vault, beside the file that shows them
  lib/components/        sidebar tree, post/prompt/feedback/upload cards, composer, modals, hotkey recorder …
  routes/screenshot-overlay  the built-in capture's region picker (its own window; not used on Linux)
scripts/release.mjs      the next version from the conventional commits (run by CI)
scripts/check-commits.mjs  checks every commit in a pull request against CONTRIBUTING.md (run by CI)
scripts/conventional.mjs what counts as a conventional commit, shared by both
src-tauri/src/
  schema.sql             the in-memory database's shape and rules
  projects.rs threads.rs posts.rs prompts.rs feedback.rs uploads.rs deliverables.rs templates.rs
  provenance.rs          how published versions came to be: the records, what old versions prove, the "Produced by" window
  stories.rs board.rs search.rs links.rs
  reviews.rs             code reviews written with margin-code: read leniently, never written
                         one store per concern, plain functions over the in-memory connection
  vault.rs frontmatter.rs   files ⇄ database: layout, lenient import, canonical export, write-through
  settings.rs            vault location, .ledger/settings.json and .env, .gitignore
  sync.rs                the three-way reconcile, behind a `Remote` trait
  github.rs              the GitHub REST implementation of `Remote`
  syncer.rs              the background worker: debounce, poll, retry, status events
  watcher.rs             watches the vault folder and reloads when its files change outside the app
  screenshot.rs          hotkey, your own tool, pictures opened with Ledger, the built-in capture; Linux portals
  updates.rs             looking for releases, the notice's view of one, and when it is safe to install and restart
  state.rs commands.rs   the running app and the thin Tauri command layer
  markdown.rs            markdown → HTML; raw HTML is escaped and unsafe link targets dropped; [[links]] are marked for the page to resolve
  tests*.rs              behaviour tests
```

## Not carried over from the legacy app

The read-only MCP server, PDF export, and the backfill command for old version notes. There is no migration: the legacy content repository used `meta.json` and `*.json` files and a different structure, and Ledger starts from an empty vault.
