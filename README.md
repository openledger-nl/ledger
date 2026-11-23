<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src/lib/assets/brand/svg/ledger-wordmark-on-dark.svg">
    <img src="src/lib/assets/brand/svg/ledger-wordmark-on-light.svg" alt="Ledger" width="420">
  </picture>
</p>

A local desktop app for thinking in **threads**, asking for **deliverables** and giving feedback on them, and planning the work on a **board** — all inside **projects**. Tauri 2 (Rust) on the back, SvelteKit (Svelte 5) on the front. Your data is a **vault** — a folder of markdown files with YAML frontmatter, like an Obsidian vault, or of encrypted files — that your cloud storage (OneDrive, SharePoint, Nextcloud) keeps in sync and backs up.

This is the rewrite of the original ASP.NET ledger, which is kept in the author's private repository (MilanMolenbrugge/ledger, commit `3ec6769`). The data model carries over in spirit, and so does the idea of plain files; the shell does not. The look follows the Leerjournaal app: a dark sidebar, calm cards, one accent colour, six themes.

## How it keeps your data

```
   the vault (files)  ──load at start──▶  in-memory database  ◀── every read
         ▲                                       │
         └────────── every change, written through ┘
         │
         └── cloud storage (OneDrive, SharePoint, Nextcloud): carries the folder to your other computers,
             keeps earlier versions of every file, and can put the whole folder back as it was on a day
```

- **The vault is the source of truth.** A vault of plain files opens in Obsidian, grep finds what is in it, any editor
  changes it; an encrypted vault (see below) is readable only by Ledger. Ledger watches the folder: what the cloud
  brings in from another computer, or another program writes, is read in once the folder is quiet again (and when the
  window regains focus).
- **At startup** the whole vault is read into an in-memory database. Screens are answered from memory; every change is
  written straight back to the files: only the files that actually changed, and atomically, so the cloud uploads only
  what changed and never sees half a note.
- **Your cloud storage is the backup.** Ledger doesn't sync anything itself. Put the vault in a folder your cloud
  storage keeps in sync, and its version history, its recycle bin and "restore this folder to a day" (OneDrive's
  Restore your OneDrive, SharePoint's Restore this library) are how a mistake is undone.
- **It looks before it writes.** A file that changed on disk since Ledger last read it (another computer's change
  the cloud just brought in) is not overwritten: Ledger keeps what it had beside it as a conflict copy and reads the
  vault again. A copy the cloud made itself, when two computers changed one file at once (OneDrive's
  `name-COMPUTER.md`, Nextcloud's and Dropbox's "conflicted copy", Syncthing's `.sync-conflict-`), is recognised too:
  it is not read in twice, the sidebar says how many wait, and the **Conflicts** page shows both versions and keeps
  the one you choose. A write that a sync client holds up for a moment (Windows refusing to replace a file it has
  open) is tried again.

### The vault

```
projects/<project>/project.md                       a project (name, board columns; its kind, goal, end and intended AI-use label; who is part of it, by id, each with their role in it; each deliverable's number, goal and own label; the branding of its PDFs)
projects/<project>/README.md                        the project's reading guide — where to start; in a study project linked to
                                                    Portflow, the description of its collection (kept in step, not copied)
projects/<project>/threads/<thread>/thread.md       a thread
projects/<project>/threads/<thread>/posts/<id>.md       its posts, each with its kind and labels when you set them; a "version
                                                        published" note is a link to the version's manifest and nothing more
projects/<project>/threads/<thread>/prompts/<id>.md     its prompts: requests for deliverables
projects/<project>/threads/<thread>/feedback/<id>.md    its feedback: on a deliverable, a user story or the project
projects/<project>/threads/<thread>/uploads/<id>.md     upload manifests: when, where the file is, a description
projects/<project>/threads/<thread>/runs/<id>.md        a run of a prompt with Claude Code: how it ended, what it did
projects/<project>/userstories/<id>.md              a user story
projects/<project>/tasks/<id>.md                    a task (a card on the board)
projects/<project>/deliverables/                    what prompts produce — any kind of file; only the live files
projects/<project>/deliverables/…/<name>.desc.md   a deliverable's evidence description, beside it (no deliverable of its own)
projects/<project>/versions/<deliverable>/          a deliverable's published versions, in a folder named after it at the
                                                    same place under versions/ as its live file is under deliverables/:
                                                    name@v0.1.ext, the version itself, tagged
                                                    name@v0.1.manifest.md, how it came to be: who made it, the thread,
                                                    prompt and run behind it, its AI-use label (and the intended one, with
                                                    why, when they differ); name.ext@v0.1.manifest.md when two
                                                    deliverables share the folder (plan.md and plan.pdf)
projects/<project>/evidence/<item>.md              a Portflow evidence item (mirrored): its outcomes, versions and their files
projects/<project>/attachments/                     files uploaded to a thread, screenshots included
projects/<project>/branding/                        the logos on its PDFs' covers: own.png (yours) and client.png
projects/<project>/exports/                         PDFs of markdown deliverables: name v0.1.pdf for a version, name (draft).pdf
                                                    for the live file; made again whenever you export
projects/<project>/templates/prompts/<id>.md        templates for this project
projects/<project>/templates/pdf/layout.typ         this project's own PDF layout (Typst), with the files it uses beside it
meetings/<YYYY>/<MM>/<YYYY-MM-DD>.md                a day's meeting notes, one heading per meeting (its time and projects in the frontmatter) — written as Obsidian writes daily notes
shared/templates/prompts/<id>.md                    templates for every project
shared/templates/pdf/layout.typ                     the vault's own PDF layout (Typst), with the files it uses beside it
shared/templates/semesters/<id>.md                  a semester template, in the form of a semester's file: what a new
                                                    semester can start from (a new vault has nj26.md)
shared/semesters/<id>.md                            a semester (half a year) and its sprints
shared/people/<id>.md                               someone the vault knows: name, email, organisation, notes
shared/clients/<id>.md                              a client work projects are done for: name, website, notes
shared/portflow/                                    what Portflow mirrors: the learning outcomes, people and sections
ledger.json                                         the marker: that this folder is a vault, its format and its id (and, for
                                                    an encrypted vault, its key sealed with the passphrase)
```

Files Ledger doesn't know about are left alone, so the vault can hold other notes. Hand-written files are read leniently: a folder of posts without a `thread.md` is a thread, a post without frontmatter is all body, links to things that don't exist are dropped, and the repairs are written back. Files dropped into `deliverables/` by hand (or by an assistant) show up in the app.

An encrypted vault holds `ledger.json` and a `data/` folder of sealed files only; see below.

**Ledger 1.0 makes a clean start**: it opens only folders with a `ledger.json` of a format it reads. A vault made by
Ledger 0.x (no marker, a `projects/` or `.ledger/` folder) is refused and left as it is: make a new vault. A vault of
today's that has lost its `ledger.json` (it has a `shared/` folder), or whose `ledger.json` hasn't synced to this
computer yet, is refused too, and the picker says so: wait for the sync, or put the file back from the cloud's version
history. Ledger doesn't make it a new one, which would give it another id. When the
format changes later, `ledger.json` says which one a vault is in, and a step that runs once brings it up to date.

### Choosing where the vault lives

**The first start** asks for a vault, as Obsidian does: **create a new vault** (a name and a place, Documents by default,
and whether to encrypt it) or **open a folder as vault** (one that is a vault already, synced here from another computer,
say). A new vault is never made in a folder that already holds files. Keep the vault in a folder that cloud storage
keeps in sync (OneDrive, SharePoint, Nextcloud), so the files are safe if something happens to the computer: Ledger
suggests it, but doesn't check. Every later start opens the vault used last.

**Switching vaults**: the vault's name at the bottom left of the sidebar (or the vault button at the bottom of the icon
rail), or **Settings → Vault → Switch vault**, opens the same picker over the window: the vaults opened on this machine,
the latest first (an X takes one off the list and leaves its folder alone; a lock marks an encrypted one), next to
creating and opening one. Opening a vault reloads the window on it. **Settings → Vault** can also move the open vault
to another folder (optionally copying it there when that folder is empty).

**What belongs to this computer** is never in the vault, so the cloud never uploads it: the open vault and the recent
ones (`config.json` in the app's config folder), and, by the vault's id, this computer's settings for each vault in
`vaults/<vault id>/` beside it (the screenshot hotkey and tool, the folded sidebar, Claude Code's, Trello's and
Portflow's settings and bookkeeping, and what those syncs set aside). The secrets (the Trello key and token, the
Portflow session, an encrypted vault's key) go to the system's keychain; only where there is none (a Linux without a
Secret Service) do they go to `secrets.env` in that folder, readable by this user alone.

### Encrypted vaults

Tick **Encrypt this vault** when you make one, and choose a passphrase. Every file is then kept on disk, and so in the
cloud, only sealed: AES-256-GCM, under a name made from its path with the key (`data/3f/3f9a…e1.lgr`), so neither what is
in the files nor what they are called can be read; the file's path is sealed inside it.

- **The key.** Each vault has its own random key. `ledger.json` holds it sealed with a key made from the passphrase
  (Argon2id), so another computer opens the vault with the passphrase. The **recovery key** (the key itself, as text,
  shown once when the vault is made, and under **Settings → Encryption** for the passphrase) opens it when the passphrase
  is lost: keep it apart from the computer. Without either, nobody can open the vault, Ledger included. A computer that
  opened the vault keeps the key in its keychain, so it doesn't ask again (**Forget the key on this computer** undoes
  that). Changing the passphrase seals the same key anew; the files stay as they are.
- **Plain only in memory.** The files are decrypted when Ledger reads them, into memory. A program outside Ledger that
  needs a real file (a picture or a PDF opened with its own program, a PDF layout opened to edit, a Claude Code run,
  which gets a copy of its project) works on a plain copy in a work folder in the system's temporary folder, outside the
  synced folder; what it changes there is taken back in, sealed, when Ledger's window is back in focus or the run is
  over, and the work folder is wiped when the vault opens again.
- **What it costs.** Obsidian, an editor or grep can't read an encrypted vault, and restoring one file from the cloud's
  web page is hard to do by name (restoring the whole folder to a day works as ever). Only the files that changed are
  sealed again, so the cloud uploads only what changed. What stays visible: that it is a Ledger vault, how many files
  it holds, roughly how big each is, and when they change.
- **Encrypting a vault you have** happens in a copy: **Settings → Encryption → Make an encrypted copy** writes it into a
  new folder and opens it. The vault of plain files is left as it is; delete it yourself once the copy has synced, and
  remember its files stay in the cloud's recycle bin and version history for a while.

## What it does

- **Getting started** — a client with a vault that has nothing in it yet opens on the home page, where a guide offers two ways in. **Import an existing vault** from its folder on the computer, synced there by your cloud storage. Or **set up a new one** in steps that tick themselves off as they are done: what you want to use Ledger for first (study, work or a hobby), a semester for study only (from a semester template, NJ26 in a new vault, or your own), a first project (in the New project window, its kind already the one you answered), a first thread, and a first deliverable, which ends the guide. The semester can be put off, and what it is for changed. A new vault asks again. A vault that already has a project or a semester never gets the guide, and **Settings → Vault & external → Vault** brings it back.
- **Projects** — each has a reading guide (its `README.md`, rendered and editable on its Overview tab, under the project's latest threads and deliverables with buttons for a new thread and a new deliverable), threads, deliverables, user stories, a board, templates and its people (coach, teammates, stakeholders, from the people of the vault, each with their role in this project: offered first wherever someone is named in it), each on a tab of its own. **Edit** on its page opens the project's window (its name, kind, goal and end, intended label, main question or client, and people), and its **...** menu deletes it. In the explorer on the left a project unfolds into its threads and deliverables, with a button for a new thread and a new deliverable; opening another project folds the first. Clicking a project always goes to its page; only on that page does clicking it again fold it, and the arrow before it folds and unfolds it anywhere. Deleting a project removes its folder. On a project linked to a Portflow collection (a study project, or one without a kind yet), the reading guide *is* that collection's description in Portflow: one text, called the reading guide (README.md) in Ledger. The first sync takes the description in (a guide that said something else is kept in `.ledger/conflicts/`), and every later one takes a change made in Portflow into the README; with **Reading guide → collection description** switched on under Settings → Portflow (off by default, and only with read and write on) a change made in Ledger goes back, and when both changed Ledger's text wins (with the write off it stays in Ledger and the sync report says both changed). There the guide comes first on the Overview (an empty one says plainly that the portfolio needs it), with **Hand in to Portflow…** (it copies the guide and opens Portflow, where it is added as a text). Its `@` mentions of evidence items stay mentions: in the README they are written `[@Analysis](portflow:evidence-2110705)`, shown as a chip that leads to the item's thread (or to Portflow when it has none), and typing `@` in the guide's editor offers the project's evidence items; a `[[link]]` to a thread that holds an evidence item reaches Portflow as a mention of that item. Projects said to be work or hobby ones are never kept in step, linked or not. With the write off the guide is still edited in Ledger as always.
- **Threads** — one per topic you're juggling. A timeline of markdown posts (replies quote their parent; edit and delete), prompts, feedback and uploads, grouped by day. Threads can be linked to each other, in both directions, and to user stories.
- **Prompts** — a request for a deliverable, written in a thread, in one dialog wherever it is made (a thread, a deliverable's Prompts window, feedback you have selected). It starts with the **deliverable**: an existing one (the dialog opens on the one you are editing), which offers its open feedback to resolve, one piece or **all of it**; or a new one, which can be given a file name (blank: whatever produces the prompt names the file) and started from a **template**, whose guiding questions are asked right there in the dialog, each to answer or leave blank to skip: what you answer goes into the prompt's base (threads with question posts from before keep them). The new deliverable dialog asks them too, and puts what you answer in the file as a `%%` note for whoever writes the first draft. What a prompt says is two texts: a **base prompt**, filled in from the template's structure or from the feedback (each piece with the version it was given on) and folded away until you want to edit it, and **what you want**, in your own words. A prompt has no title to write: Claude Code names it from what it asks, once, in the background (a small model, no tools, nothing of the vault in reach; Settings → Claude Code turns it off), and until then it is called by the start of its text. A prompt belongs to a **deliverable**, not to one of its versions: the deliverables it is about are changed under Edit, and the versions it produces know the prompt and the run in their own records. A run with Claude Code is told which versions the deliverable has, as files to read and never to change.
- **Deliverables** — files in the project's `deliverables/`. Markdown ones are edited in the app, autosaved, in live preview, source mode (the raw markdown) or reading mode, as in Obsidian: one icon at the top right switches between reading and editing (Ctrl+E), and **Source mode** is a choice in the ⋯ menu; any other kind opens with the system's program. **Publish** a version (minor, major or a custom `M.m`) to freeze an immutable copy named `name@v0.1.ext` in the project's `versions/`, which also drops a note in the thread of the prompt that produced it. Keeping the versions there leaves `deliverables/` with only the live files, so it reads cleanly in Obsidian or a file browser. Each deliverable has a side window on its right, with a button on the rail beside it for **Produced by** (see below), for **Feedback** (what you wrote on the file, by thread and under each by version, and a box to add more) and for **Prompts** (the ones about this file: make one, edit it, run it, add feedback to it): one is open at a time, and its button or the X closes it. The versions are switched in that window: click **Live** or a version's chip there. A **⋯** menu next to Publish holds Copy markdown, Open in editor, Show in folder, Export as PDF and Delete deliverable. **Export as PDF** sets a markdown deliverable (the live file, or the version you are looking at) as a report with Typst, built into Ledger, so nothing else needs to be installed: a cover with the title (frontmatter `title`, else a first `#` heading, else the file name), a subtitle (frontmatter `subtitle`, else the deliverable's goal), who it is for (the project's client), the version and its date; then the version history from the published versions (who made each, its changelog and its AI-use label); a table of contents when there are three headings or more; and the document, with a header and a footer showing the page number. `%%` notes are left out and `[[links]]` are their words; pictures beside the file go along. It is saved in the project's `exports/` folder and opened in your PDF viewer. What the cover says and how it looks is the project's, under **On exported PDFs** on its Templates tab: your name (in the header and as who made each version), your school or company and its logo, the client (the project's client unless you name another) and their logo, the colour of the headings and rules, and whether the cover shows the AI-use label (the version's Me & My Machine label, bottom left; a draft shows the one it is meant to get), which is on by default in a study project. **Take your side from another project** copies your name, your own side's name and logo, and the colour, so a new study project gets your school's logo at once. It is kept under `branding` in `project.md`, the logos in the project's `branding/` folder. The logos are on the cover (the client's top left, yours top right) and small at the left of every page's footer. **The layout itself can be changed**: it is a [Typst](https://typst.app/docs) file. **Customise for this project** (or **for every project**) under the same heading copies the layout in use to `projects/<project>/templates/pdf/layout.typ` (or `shared/templates/pdf/layout.typ`), where you change anything (the cover, fonts, margins, header and footer) and export again to see it; **Open** and **Show in folder** find it, and **Back to the standard layout** deletes your copy. A project's own layout comes first, then the vault's, then Ledger's. Other files in that `templates/pdf/` folder can be used by their name: pictures, other Typst files to import, and fonts (`.ttf`, `.otf`). The file starts by explaining what Ledger hands it; a mistake in it is reported with the file and the line. Publishing offers the same: **Export the version as a PDF and open it**, ticked unless you unticked it last time. Open feedback can be ticked, several pieces at once, and made into one prompt or added to a prompt that is there. Everything done on the page goes to one **thread**, chosen in the box at the top and remembered for that deliverable: the note a publish leaves, new feedback, and the prompts made from feedback. Feedback is on the whole document, or on a **passage**: select text in any mode (source, live preview or reading) and click the comment icon after the selection. The passage is kept as plain text with a few words around it (`quote`, `quotePrefix` and `quoteSuffix` in the feedback's file), so it is lit up in the document wherever it is found again, a click on it shows its feedback, and the feedback says so when the text it was about has changed. In a project linked to Portflow (a study project, or one without a kind yet), **every evidence item is a deliverable**, one per item and named after it: the sync downloads each version's file once and publishes Portflow's version N as `v N.0` (its record says "From Portflow, version N", in the item's thread), so "Project plan DAE" with the file `Project plan.pdf` is `deliverables/Project plan DAE.pdf` with `versions/Project plan DAE@v1.0.pdf`. Of a version with several files, the one named most like the item is the deliverable and the others are its attachments, in `deliverables/<item>/` and linked from its page; a later version under another file name is the next version of the same deliverable. The newest version is the live file, unless that was changed in Ledger (then it stays, and the sync says so). A file Ledger handed in itself is linked, not downloaded again, and keeps its own name; one from Portflow follows the item when it is renamed there. A download that fails is retried on the next sync. A hobby or company project keeps its evidence files in Portflow.
- **Produced by, and AI use** — every published version records who made the change: you (you typed it), Claude Code (a run started from the app, which also links the version to its prompt), or AI you say you used elsewhere. The **Produced by** window on the right of a deliverable opens with a summary, then lists the live file as the newest version (a draft the moment it holds more than the last one, as you type), then its threads and under each thread what happened there, in the order set under Settings → Order (newest first unless you change it): the prompt that asked for the file, the run, each version with who made it, its size and a SHA-256, and the feedback on it. The "version published" note in a thread links to the prompt that caused the version, and quotes the changelog you wrote for it ("from changelog: …"), if you wrote one; its file is only a link to the version's manifest, so what it says is read from the manifest and can't disagree with it. When you publish, a checkbox says the change was made with generative AI (it starts ticked when a Claude Code run changed the file and nothing published it), and you can attach a label from the [Me & My Machine](https://mmmlabel.tech) scheme — Lazy Prompter, Piggybacker, Cyborg, Handyman or Man-Made. Ledger records the facts; the label is yours to declare, per version, because it describes the product as it stands then. Versions published before this existed show what the runs and notes prove (versions a run published are known to be AI) and otherwise "origin not recorded". On any version, click who made it (**You** or **AI**) or its label to change it; a version a Claude Code run made keeps its author. The records are plain files, so they can be edited by hand.
- **Changes while you write** — the live file of a markdown deliverable shows what changed since its newest version (or any older one, picked in the **Changes** section of the side window), in the text itself as Word's track changes: added words underlined and removed ones struck through where they were, green where you changed it and in the accent colour where AI did, with a thin bar in the margin beside each changed passage and a notch where one was taken out. A click on a change opens a card with what was there before, who made the change and **Undo this change** (Ctrl+Z brings it back). After a version AI made, a line above the text offers to **Review** its changes: they are gone through one by one, each kept or undone, until a small bar at the bottom says none are left. The side window's Changes section also turns the marks in the text and the bars in the margin on and off (kept per machine), and opens the views below. Nothing of this is stored: it is worked out from the versions as you type.
- **What changed, and who wrote what** — two views over the whole text, opened from the side window's Changes section (Close, or Ctrl+E, goes back to the text). **Changes** compares any two versions, or a version and the live file: as the document reads, with added words underlined and removed ones struck through as in Word's track changes, or **line by line** as git shows it; a passage that only moved is shown as moved, and **Only the changes** folds away what stayed. What was added is coloured by who added it, you or AI, so a comparison over several versions still tells them apart. **Who wrote what** shows one version (or the live file) with what AI wrote softly tinted and a line in the margin beside its paragraphs, and how much of the text each of you wrote; **Light up** one version to see where its words ended up: how many of them are still there, and who took the rest out. The arrows step through the changes or passages. Nothing extra is stored for this: who wrote a passage is the version it first appeared in, which is exact because a Claude Code run always gets a version of its own (your edits that no version holds are published before it changes the file). The **Produced by** window links to it from each version ("What changed since v0.2", and "Where its words are now" for a version AI made). For Obsidian, each published version of a markdown deliverable also gets a note in the project's `history/` folder, `history/<deliverable>/v0.3.md`: who made it, its prompt and changelog, the words added and removed since the version before, and the changed passages with `==added==` and `~~removed~~` text (code and tables as a `diff` block). Ledger writes those notes from the versions and writes them again when they change.
- **Meeting notes** — one note a day, in a folder per year and month (`meetings/2026/11/2026-11-20.md`), written as Obsidian writes daily notes, so the same note opens in both: set Obsidian's Daily notes plugin to the folder `meetings` and the date format `YYYY/MM/YYYY-MM-DD`. Each meeting is a `## Title` heading with a `With:` line linking to the people who were there (`With: [[people/frank]], [[people/sam]]`), then your notes; its time and the projects it belongs to are in the frontmatter, under `meetings` (`- title: Title`, `time: 10:00`, `projects: [x]`, found by the heading's title), and its tags gather the projects (`project/x`), so a day is found by project, and by person through the links. A meeting's notes link as every other field does, from the meeting's project when it has just one (with several or none, a link to a project's thread or deliverable names the project), and `@` writes a link to someone. The people of a meeting's projects are offered first, **Whole team** adds every teammate in one click, and someone typed who isn't in the vault yet gets a file in `shared/people/` when the note is saved. Saved as you type; text and frontmatter of your own in the note are kept, and a day without meetings has no file.
- **Feedback** — written in a thread, landing on a deliverable or on one of its published versions; open or resolved. The **Feedback** inbox lists every piece of feedback across the projects (Portflow's too), filtered by who gave it, by source and by project, and resolves it in place. **Record feedback** there writes down your own note or what someone gave you (in person, in a meeting, by email or by phone, with their role), on a deliverable or version, a user story or the whole project, kept in a thread of your choice (the most recent one, or a new “Feedback” thread); its file says `kind` (own or someone), `story` and `daily` (the meeting note's day).
- **Uploads** — pick files in a thread, or drag them onto the window from anywhere: in a thread they are attached to it at once, elsewhere Ledger asks which project and thread they go to. They are copied into the project's `attachments/` and noted in an `uploads/<id>.md` manifest with a description. Screenshots land the same way. A project's **Attachments** tab is its `attachments/` folder as a document library: every file in it, the ones uploaded in its threads and the ones you put there yourself (a PDF someone sent you), folder by folder, with the path to the folder on top. Sort by name, date or size, show a list or tiles with a preview for pictures, filter to images or other files, or filter by name, description or thread across every folder; open a file (a markdown file is read in Ledger itself, rendered, with its links and pictures, and its properties folded away; anything else opens in its own program), show it in its folder, or go to the thread it was uploaded in. **Open folder** opens the folder you are in in the file manager to put files in, and what you put there shows up when you come back to Ledger (in an encrypted vault that folder is a plain copy outside the synced folder, taken in the same way). Ledger never deletes a file in `attachments/`: removing an upload, or the thread it is in, removes only its manifest, and the file stays until you delete it yourself.
- **Templates** — a name, a "use when…" hint, guiding questions, the skeleton of the deliverable, the kinds of project it is for and its **readers**: who the deliverable is written for. Global ones (Shared → Templates in the explorer, with the semester templates) serve every project of their kinds (study, work, hobby; none picked is every kind); a project can *customise* one, which copies it into the project, where it replaces the shared one. A reader is a preset (Me, later; the project's people, read as their roles, a stakeholder as the client; Client; Coach or assessor; Teammates; Developer or maintainer; Outside reviewer) or someone in your own words, each with what they need from it and what to keep from them. A prompt from the template tells Claude who the readers are, in its base (so a prompt can name others), and a new deliverable notes them under its title as `%% Readers: … %%`, or also as a visible `**Readers:**` line when the template says so. Write a template's guidance to the writer between `%%` marks, as in Obsidian: Claude follows it and leaves it out. A new vault starts with one, a basic **Project plan** (`shared/templates/prompts/project-plan.md`): the vault's own from the start, to change or delete like any other. In the files a template's frontmatter has `kinds`, `readers` (each `who`, `needs`, `keepFrom`, or just the `who`) and `readersShown`. Under the templates, a project's Templates tab also holds **On exported PDFs**: the logos, names, colour and layout its deliverables' PDFs get (see Deliverables).
- **User stories** — an id you choose (`MM01`), a summary, "As a / I want to / Such that", notes and acceptance criteria you tick off. A story links to any number of threads.
- **Board** — per project, with customisable columns (Backlog → Doing → Done by default). Tasks can point at a story and a thread. Drag a card, or use its arrows, to change column; a column that still holds tasks can't be deleted.
- **Screenshots** — press a global hotkey (default Ctrl/⌘+Shift+S) anywhere: your own screenshot tool (Flameshot, say) or the built-in capture takes the picture, and Ledger offers to attach it to the thread you have open. Pictures sent to Ledger with "Open with" land the same way. See below.
- **Writing** — every markdown field (notes, prompts, feedback, descriptions, deliverables, templates, the reading guide) is the same inline editor ([CodeMirror 6](https://codemirror.net)). It edits the markdown itself, so the file gets exactly what you typed, but reads like the rendered page: headings, bold, code and links are shown as such and their marks fade away except where the cursor is, tables are drawn as tables until you click into one, and pictures are shown in place. Notes between `%%` marks (Obsidian's comments) are only seen in source mode, faded; live preview hides them unless the cursor is in one, and the rendered page leaves them out. Task boxes can be ticked, Ctrl/⌘+click opens a link, and Ctrl/⌘+B, I and K make bold, italic and a link. **Links between things** are written as in Obsidian, in every one of those fields: `[[` offers the project's threads, deliverables and feedback, the days of the meeting notes and the vault's people (type to narrow the list, Enter picks), and another project's things once a name of theirs or the project's is typed; the brackets close themselves. `[[Pricing model]]` is a thread, by its title; `[[pricing-brief]]` a deliverable, by its file name (`[[plans/pricing-brief.md]]` by its path, and a file that isn't markdown keeps its extension); `[[feedback:<id>|too long]]` a piece of feedback, which has no name, so the link carries words of its own (the list writes them from what the feedback says); `[[2026-11-22]]` the meeting notes of that day, as Obsidian links a daily note; `[[people/frank]]` a person, their file in `shared/people/` (shown as their name). `[[brief.pdf]]` is a file in the project's `attachments/` folder, by its name (`[[attachments/hans/brief.pdf]]` by its path), and following it opens the file in its own program. Another project's thread, deliverable or feedback is written after that project's slug, `[[foo-bar/pricing-brief]]`, and only then: a link is read from the project it is written in (a meeting's from its project when it has just one, so a meeting of several projects, or none, names the project every time). `[[target|words]]` shows other words; `[[thread:Name]]` and `[[deliverable:name]]` spell out the kind when a thread and a deliverable have the same name. In live preview a link reads as its words, and its brackets show where the cursor is; Ctrl/⌘+click follows it, and on the rendered page a click does. A link that points at nothing is faint and dashed. A link finds things by what they are called when it is followed, and renaming a thread or a deliverable writes every link that meant it anew, its words kept; a published version keeps its links as they were when it was published. The links stay as typed in the files; Obsidian reads them as its own links, which open there only when a note of that name exists. In a vault-wide template there is no project to point into, so nothing is offered there.
- **Dashboard** (Home) — a card per project, the most recently active first, with its three latest threads and two latest deliverables, and links to its board, a new thread and a new deliverable. **Pin** a project (the pin on its card) to keep it at the top. Above them, a contribution graph like GitHub's: a square per day, shaded by how much you made that day, for all projects or one. It opens on the semester you're in (or the nearest one; with none set up it says so and links to the Semesters page), every day of it drawn: today has a ring, the days that haven't happened yet are dashed and empty, and the stats count the days left. Or pick a year you made something in. It counts what you make: threads, your posts, prompts, feedback, uploads, published versions, Claude Code runs, stories and tasks; not edits, which leave no history, nor what the app writes for you. Switch it off under Settings → Dashboard. Pins and that setting are kept per machine, like the theme. The explorer's footer says how Portflow stands when Portflow is on, how many conflict copies wait when there are any (a click opens the Conflicts page), and under them the open vault's name, which switches vaults.
- **Shared** — what every project shares has a heading of its own in the explorer, with two entries, **Semesters** and **People**, each leading to its page (not the semesters or people one by one).
- **Semesters** (Shared → Semesters in the explorer, or the Semesters button on the dashboard's contribution graph) — half years you set by hand, listed there, with a + to add one: from a **semester template** (a new vault has **NJ26**: autumn 2026, a kick-off week, a shorter sprint 0, five sprints of three weeks, a week off in autumn and two at Christmas; named "NJ26 - Semester X" until you fill in the X) or of your own: a name, a first and a last day, and the vacations in them. **Save as template** keeps a semester's days, vacations and sprints to start the next one from; the templates are files of their own in `shared/templates/semesters/`, listed, changed and deleted under Shared → Templates, and a semester made from one is its own. Each is cut into sprints of three weeks of work (or another length for all, and any sprint can have a length of its own) from its first day. The sprint clock stops for a vacation: a sprint that runs into one carries on after it, one that would start in it starts after it, and less than a week of work left at the end joins the last sprint. So the sprints always fit; you name them and give them goals. Semesters can't share days, so a day belongs to one semester at most. Kept as `shared/semesters/<id>.md`; on the dashboard's graph vacation days are outlined.
- **People** (Shared → People in the explorer) — everyone the vault knows: coaches, teammates, stakeholders and anyone else, one file each in `shared/people/<id>.md` (a name, an email and organisation if you like, a Portflow id, notes). A person is the vault's, not one project's: the same person in every project they are part of, and shown only in the projects you add them to, on the project's People tab, with the role they have there (Frank can be the coach of one project and an expert in another). The page lists everyone with their projects, and adds, changes and takes people out of the vault (which takes them out of every project). Markdown links to a person as `[[people/<id>]]`, in any field and in a meeting's `With:` line, so their file shows where they come up as backlinks in Obsidian; the id is their name as a slug when they are added, and stays when they are renamed. Feedback says who gave it by name.
- **Clients** (Shared → Clients in the explorer) — the organisations work projects are done for, one file each in `shared/clients/<id>.md` (a name, a website, notes). A work project names its client by id: pick one in the project's Edit window, where a new name is added to the vault. The project's header links to its client, and the client's page lists its projects; taking a client out of the vault leaves its projects without one. The people who speak for a client are the project's stakeholders.
- **Dates** are shown the European way everywhere: day before month (3-11-2026, typed as 03-11-2026; written in full, 3 November 2026, on a PDF) and a 24-hour clock, in Amsterdam time whatever the computer's own time zone. Files keep the ISO form that YAML and other tools expect: a day as 2026-11-03, a moment in UTC as 2026-11-03T13:05:00.000Z; a hand-written semester may use either form of a day. One module on each side holds these rules: `src/lib/format.ts` and `src-tauri/src/dates.rs`.
- **Command palette** (Ctrl/⌘+P): type a few letters of a command or a page and press Enter. It finds the commands (start a meeting note, new project, new thread or deliverable in the open project, take a screenshot, switch vault or theme), the main pages and settings tabs, every project and its tabs, and every thread. Any of them can have a keyboard shortcut of its own (**Settings → Shortcuts**, one page with every command and its key, kept per machine in the window's storage): Ctrl/⌘+Shift+D starts a meeting in today's note, Ctrl/⌘+K searches, Ctrl/⌘+\\ folds the sidebar and Ctrl/⌘+Alt+Shift+D starts a debug capture, unless you change them. A key another command had moves over to the new one.
- **Search** across every post, prompt, feedback and deliverable, with highlighted snippets. **Themes**: Sepia (the default), Dark, Night, Forest, Ocean, Light. **Order** (Settings): one choice, newest first by default, for the posts of a thread, the threads in the sidebar, the feedback, versions and "Produced by" window of a deliverable, and a project's attachments. It only changes how things are shown. **Settings → Features** also turns the contribution graph and the explorer's **Latest** section (the newest threads and deliverables, under the projects) on and off. **Sidebar** (Settings → Visual): what the sidebar becomes when you fold it (Ctrl \\) or the window is narrower than 1200 px, an icon rail (the default) or nothing; below 720 px it is always a drawer behind the menu button. Kept for this computer, outside the vault, so each computer has its own. **Name prompts** (Settings → Claude Code, on by default): a new prompt is named by Claude Code from what it asks. **Run prompts by themselves** (Settings → Claude Code, off by default): a prompt you create starts running in Claude Code at once, without the confirmation of what will be sent. **Model** (Settings → Claude Code): the model runs use, chosen from Opus, Sonnet and Haiku (Claude Code's aliases for the newest of each), Claude Code's own default, or any name it accepts; the confirmation before a run can choose another for that run only.

## Screenshots

**Settings → Screenshots** holds the hotkey and the tool. Both are this computer's settings for the vault, outside it, so each machine can use its own tool. The hotkey is set with a key recorder, checked (it needs a modifier, and the system's own shortcuts are refused) and registered immediately, replacing the old one without a restart; if the system won't register it (usually because another program owns it) you get a warning and the old shortcut keeps working.

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

Installing downloads the new version and checks that it is signed with Ledger's key (the public half is built into the app, `plugins.updater` in `src-tauri/tauri.conf.json`); an update with a wrong or missing signature is refused, and the notice says so. It then waits until a restart cuts nothing off: a Trello sync that is running finishes first, and if Claude Code is still running a prompt, you choose between waiting for it and stopping it. Then Ledger installs and restarts. Every change you make is written to the vault straight away, and your cloud storage takes it from there. An update replaces the program only: the vault, this computer's settings for it and Ledger's config are left alone.

**Settings → About** shows the version you have and has **Check for updates**. To stop the automatic checks, untick **Check for updates automatically** there; **Check for updates** still works. The choice belongs to this computer, not to the vault: it is kept in Ledger's `config.json` in the app's config folder (next to the vault's location), so another computer syncing the same vault keeps its own.

Ledger built from source with `pnpm run dev:app` (scripts/dev.mjs) runs from your checkout as `pnpm run tauri dev` does, but nothing that changes there is put in front of you until you say so. It keeps watching the files: a change to the front end doesn't reload the window, and a change to the Rust side is built in the background while the app keeps running the build it started with. Ledger then shows what is waiting: the commits the checkout's branch gained since the running build (merged by you, or by an assistant working in another worktree; read from the local branch, nothing is fetched), and whether the new build is still being built, ready, or failing. Only **Update** brings it in, once a sync or a Claude Code run has finished: the window reloads when only the front end changed, otherwise Ledger restarts into the new build (and installs the packages first when `package.json` changed). **Not now** keeps the prompt away until something else changes. Quitting Ledger ends `pnpm run dev:app`; starting it again runs whatever the checkout has. Plain `pnpm run tauri dev` still reloads and restarts by itself as files change, and says nothing; a debug build that isn't served by the dev server (`tauri build --debug`) never looks. A release build you make yourself looks for releases, and offers the official ones.

### Building from source

You need [Node.js](https://nodejs.org) 26 (the version in `.nvmrc`; `nvm use` picks it) with [pnpm](https://pnpm.io) (`corepack enable` activates the version pinned in `package.json`), a Rust toolchain and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform (on Windows: the MSVC build tools and WebView2).

```bash
pnpm install
pnpm run dev           # the app, with hot reload (the same as pnpm run tauri dev)
pnpm run dev:app       # the app, with every change held until you choose Update in it
pnpm run tauri build   # an installer / bundle for this machine
```

While the app runs from a checkout, its front end also opens in an ordinary browser at http://127.0.0.1:1420, to test
or inspect it there. The browser tab reaches the backend through Ledger's window (`src/lib/browserRelay.ts` and the dev
server plugin `scripts/browser-relay.mjs`, over the dev server's own socket: no new port), so keep the window open.
What belongs to the window stays there: a file is dropped on the window, a dialog to pick a folder or a file opens over
it, and global shortcuts are the app's. Anyone with that dev page open can act on the vault, as in the window. Only the
development server has this; Ledger as installed is the desktop app alone.

The UI is also in [Storybook](https://storybook.js.org): `pnpm storybook` serves it at http://localhost:6006 (no app or
backend needed), `pnpm build-storybook` builds it into `storybook-static/`. Every component in `src/lib/components` has
its stories in `src/stories/<area>/`, and the main pages are there under **Pages**, drawn with their own layouts and
load functions. The data is a made-up vault (`src/stories/fixtures.ts`), and Tauri's IPC mock answers each backend call
from it (`src/stories/backend.ts`).

A local `tauri build` makes the installers but not the updater's signatures: those need the private key, which only CI has.

Checks:

```bash
pnpm test              # Rust: stores, vault round-trips, encrypted vaults, and a vault in a folder the cloud changes
pnpm run check         # Svelte + TypeScript
```

Environment variables, mostly for development: `LEDGER_VAULT` fixes the vault location (no picker, and no other vault can be opened); `LEDGER_CONFIG_DIR` moves the per-machine settings (the vault location and the recent vaults, whether to check for updates; an empty folder shows the first start's vault picker).

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

To try a version before it reaches `main`, push its branch to `alpha` (`git push -f origin HEAD:alpha`).
[.github/workflows/prerelease.yml](.github/workflows/prerelease.yml) builds the next version as `vX.Y.Z-alpha.N` on the
three platforms and publishes it as a GitHub pre-release. It commits nothing and leaves `CHANGELOG.md` alone; the
release script skips its tags. Installed copies are never offered an alpha, because the updater reads the latest
release and that skips pre-releases.

## Layout

```
src/                     SvelteKit frontend (static SPA)
  routes/                / · projects/[project] (overview) with threads/[thread], deliverables(/view),
                         stories, board, templates, people · semesters · people · conflicts · search · settings
  lib/api.ts             typed wrappers over the Tauri commands
  lib/ui.svelte.ts       toasts, dialogs, theme, and the backend's vault and screenshot events
  lib/vaults.svelte.ts   the vault picker's overlay: opened from the sidebar, the rail and Settings
  lib/commands.svelte.ts the command palette's commands (pages too), their shortcuts, and the listener for them
  lib/updates.svelte.ts  the update notice's state, and installing (waits, then restarts)
  lib/markdownEditor.ts  the markdown editor (CodeMirror 6): looks, live preview of the marks, tables, source mode, keys, [[links]]
  lib/links.ts           [[links]]: which thread, deliverable, feedback, meeting day or person a typed target means, read from
                         the project it is written in, and what to offer after `[[`
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
  history.rs             what changed between versions and who wrote what, worked out from the versions
  stories.rs board.rs search.rs links.rs
                         one store per concern, plain functions over the in-memory connection
  vault.rs frontmatter.rs   files ⇄ database: layout, lenient import, canonical export, write-through that looks
                         before it writes
  marker.rs              ledger.json: that a folder is a vault, its format and id; vaults from Ledger 0.x are refused
  store.rs               every file of the vault, read and written by its path: atomic writes, retried when a sync
                         client holds a file, plain or sealed
  vault_crypto.rs crypto.rs  encrypted vaults: the passphrase and the key, sealed files, the work folder
  conflicts.rs           conflict copies, recognised by the names every provider gives them
  watcher.rs             watching the vault's folder for what the cloud brings in
  people.rs              the people of the vault, in shared/people
  clients.rs             the clients of the vault, in shared/clients
  semester_templates.rs  what a new semester starts from, in shared/templates/semesters
  settings.rs            the vault's location, and this computer's settings and secrets for each vault (keychain)
  screenshot.rs          hotkey, your own tool, pictures opened with Ledger, the built-in capture; Linux portals
  updates.rs             looking for releases, the notice's view of one, and when it is safe to install and restart
  state.rs commands.rs   the running app and the thin Tauri command layer
  dates.rs               dates and times: stored in UTC and as days, shown in Amsterdam time (the twin of format.ts)
  markdown.rs            markdown → HTML; raw HTML is escaped and unsafe link targets dropped; [[links]] are marked for the page to resolve
  branding.rs            who a project's PDFs name and how they look: names, logos, colour
  pdf/                   a markdown deliverable as a PDF: markdown → Typst markup (markdown.rs), the built-in Typst with the
                         embedded fonts (world.rs, fonts/), the report layout (ledger.typ)
  tests*.rs              behaviour tests
```

## Not carried over from the legacy app

The read-only MCP server, PDF export, and the backfill command for old version notes. There is no migration: the legacy content repository used `meta.json` and `*.json` files and a different structure, and Ledger starts from an empty vault.
