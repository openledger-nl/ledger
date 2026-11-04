# Plan: Portflow in Ledger

Fontys ICT keeps every student's portfolio in **Portflow** (by Drieam, `portfolio.drieam.app`), opened from Canvas.
That is where coaches and experts give their feedback, where the learning outcomes live and where the evidence is
graded. Ledger is the student's mission control for the study: threads, prompts, deliverables, versions and feedback.
Today the two don't know about each other, so every version is uploaded by hand and every comment is read in the
browser.

This plan connects them:

1. **Feedback from coaches and experts comes into Ledger** as feedback on the right deliverable and version, with who
   gave it and in what role, and replies as the thread around it.
2. **Deliverables go to Portflow**: an evidence item in Ledger (one or more files, in one thread) is delivered as the
   next Portflow version. Portflow's version numbers are Ledger's major versions; Ledger's minors are the drafts in
   between. Versions made in Portflow itself come back as majors.
3. **The collections fit in Ledger**: a collection is linked to a project; its description (the portfolio story,
   with its `@` links to evidence) is shown and edited in Ledger, with those links pointing at Ledger's evidence items.
4. **The learning outcomes always come from Portflow**: goals and sub-goals are never typed into Ledger; Ledger mirrors
   them read-only, refreshes them on every sync, and shows per outcome which deliverables and feedback count for it.
5. **Off until you switch it on, and fully off when you switch it off**: Portflow is a new, optional feature. Switched
   off, nothing of it shows in the app and nothing of it runs.
6. **Read-only unless you choose otherwise**: switched on, Ledger only reads from Portflow. Writing (uploading
   versions, editing descriptions, replying, …) is the student's choice, switched on in Settings, per kind of write.
7. **Tag who gave you feedback**: feedback you got in person (a meeting, a demo, a chat) is noted in Ledger with the
   teacher, coach or expert who gave it, picked from the people your portfolio is shared with in Portflow.

Everything below about Portflow was found on 2026-11-04 by reading the page and its network traffic in the browser,
and the public JavaScript bundles the page loads. Nothing was written to Portflow.

---

## 1. What Portflow is, as seen from its API

The web app is a React SPA on a Rails backend. It talks to a JSON REST API at
`https://portfolio.drieam.app/api/v1/…`. The client in the bundle is generated from an OpenAPI spec (≈490
operations), which makes it regular and predictable, but it is **not a public, documented API**: Drieam can change it
without notice. Ledger must parse it leniently and fail softly (see Risks).

### Data model

```
Portfolio (one per student, for the whole study)
├── Sections = semesters             "Open Learning FT Semester 3 – Fall 26"
│   └── Collections                  "Group Project - PoliceSpeedEdit", "Overview Collection", … (imported from the
│       │                            semester's template, here "OL-BCDFG-FALL26"; a collection lives in one semester)
│       ├── description              HTML; @-mentions are <span data-type="mention" data-mention-type="evidence" data-id="…">
│       ├── subtitle, color, position
│       ├── collection-goals         which learning outcomes this collection works on
│       ├── evidence                 linked evidence, per goal and "not in a goal"
│       └── comments                 feedback on the collection as a whole
├── Goal sets                        "PP-IN-MA-AV-OLBCDFG", imported from the semester template "OL-BCDFG-FALL26"
│   └── Goals (= learning outcomes)  Advise, Analysis, Design, Realise, Manage&Control, Professional standard, Personal leadership
│       ├── explanation (HTML, EN + NL), status ("not_yet_started", …), evidence_count
│       └── Sub-goals                Analysis-U1, Analysis-I1, Analysis-S1, … (description HTML, EN + NL)
└── Evidence                         "Project plan PoliceSpeedEdit"
    ├── versions                     v1 … v5, each with a description (the changelog) and a type
    │   └── files                    ActiveStorage blobs (name, content type, signed_id), preview/download via signed S3 URLs
    ├── goals / sub-goals            what this evidence is evidence for
    ├── collections                  where it is shown
    └── comments                     threaded: a root comment (often a feedback_request) and its children (replies)
        ├── comment_type             feedback_request, comment, criterion_evaluation, …
        ├── created_by {name, id, current_role: student | teacher | …}, created_by_role
        ├── evidence_version {version_number}   the version the comment is about
        └── feedback_request {invitees, invitations[completed]}   whom the student asked, and whether they answered
```

Version types: `file`, `free_text`, `url`, `video`, `lms_submission`, `badge`.

**One collection = one semester.** Each semester starts with a new section and new collections (and a new goal set)
from that semester's template. A collection never spans two semesters, so work that runs longer (PoliceSpeedEdit is in
its fourth semester) has a new collection every semester. Its description follows that semester's sprints
("Sprint 1: 21 September to 9 October"), the same sprints Ledger's semesters already cut.

Evidence belongs to the portfolio rather than to a semester: it can be linked to collections in several semesters, and
its versions carry on.

### Endpoints Ledger needs

All under `/api/v1`, paginated with `?page=&per_page=` and the `total`, `per-page`, `x-page` and `link` response
headers; responses carry an `etag`.

| What | Read | Write |
|---|---|---|
| Who am I | `GET users/current` | |
| Portfolio | `GET portfolios/{p}` | |
| Sections, collections | `GET portfolios/{p}/sections`, `…/collections`, `…/collections/{c}` | `PUT …/collections/{c}` (description) |
| Learning outcomes | `GET portfolios/{p}/goal-sets`, `…/goals`, `…/goals/{g}`, `…/goals/{g}/sub-goals` | none (read-only by design) |
| Progress per outcome | `GET …/goals/{g}/growth`, `…/goals/{g}/timeline`, `…/goals/{g}/feedback-items(/summary)`, `GET dashboard/goal-sets/{id}/progress` | |
| Collection ↔ outcomes | `GET …/collections/{c}/collection-goals` | |
| Evidence | `GET portfolios/{p}/evidence`, `…/evidence/{e}`, `…/collections/{c}/evidence`, `…/goals/{g}/evidence/in-collection/{c}` | `POST …/evidence`, `POST …/collections/{c}/evidence` |
| Evidence ↔ outcomes | `GET …/evidence/{e}/goals`, `…/evidence/{e}/sub-goals` | `POST …/evidence/{e}/link-to-goals-and-sub-goals`, `POST …/evidence/{e}/link-to-collections` |
| Versions | `GET …/evidence/{e}/versions`, `…/versions/latest`, `…/evidence-versions/{v}` | `POST …/evidence/{e}/versions`, `PUT …/evidence-versions/{v}` |
| Files | `GET …/evidence-version-files/{f}/preview-url` (signed S3 URL, 30 min) | `POST direct-uploads` (see below) |
| Comments | `GET …/evidence/{e}/comments`, `…/collections/{c}/comments`, `…/comments/{id}/children` | `POST …/comments/{id}/children` (reply) |
| People with access | `GET portfolios/{p}/shares`, `…/collections/{c}/shares`, `…/collections/{c}/shared-users`, `…/evidence/{e}/shared-users` | |
| Ask for feedback | `GET feedback-request/invitations`, `GET …/evidence/{e}/shared-users` | `POST …/evidence-versions/{v}/feedback-requests` |
| Confirm feedback you received | `GET …/comments/{id}/comment-validation` | `POST …/comments/{id}/comment-validations` (asks the teacher to confirm it) |
| Mentions | `POST …/mentions` (resolves mention ids to names), `GET …/mentions/evidence` (search) | |
| Notifications | `GET notification/notifications/unread-count`, `…/notifications` | |

### The people your portfolio is shared with

`GET portfolios/{p}/shares` lists them: per share the `invitee` (`id`, `name`, `current_role`: `teacher` or `student`,
`lms_id`, pronouns), the `share_type` (`portfolio`, or a collection or evidence item shared on its own), an
`expiration_date` (shares run out, usually at the end of the semester), and `feedback_restricted`. These are the
teachers, coaches, experts and peers who can see your work and give feedback in Portflow. They are Ledger's list of
people to tag.

### Uploading a file

Standard Rails ActiveStorage direct upload (the bundle carries `@rails/activestorage`'s `DirectUpload`, pointed at
`/api/v1/direct-uploads` with the Bearer token):

1. `POST /api/v1/direct-uploads` with `{"blob": {filename, content_type, byte_size, checksum}}` (checksum: base64 of
   the file's MD5). The answer holds a `signed_id` and `direct_upload: {url, headers}`.
2. `PUT` the bytes to that (S3) URL with those headers.
3. `POST /api/v1/portfolios/{p}/evidence/{e}/versions` with the `signed_id` and a description.

The exact body of step 3 isn't visible in the bundle (the form builds it); **capturing one real upload is the first
task of phase 0**, with the student's go-ahead, on a throwaway evidence item.

### Signing in

Portflow has no login of its own for students. It is an **LTI 1.3 tool inside Canvas**:

```
Canvas page (fhict.instructure.com/accounts/1/external_tools/23360)
  → POST portfolio.drieam.app/lti/open_id            (OIDC login initiation)
  → Canvas /api/lti/authorize (prompt=none)          (uses the Canvas session; Canvas uses Fontys SSO / Microsoft)
  → POST portfolio.drieam.app/lti/launches           (the id_token)
  → the app's HTML page: <body data-session="JWT"> and <meta name="csrf-token">
```

The SPA reads its token from `document.body.dataset.session` and sends it as `Authorization: Bearer …`, with
`X-CSRF-Token` from the meta tag. The token is a JWT (`sub: "lti-session"`, the launch id, the user id and the tenant)
that is **valid for 8 hours**. A new launch gives a new token, silently, for as long as the Canvas session lasts.

---

## 2. How Ledger signs in: your Fontys account, in a Ledger window

Because Ledger runs locally, it can open a window of its own and let the student sign in with their **Fontys account**
there, the same way the browser does. That is the plan, with one firm rule: **Ledger never sees or stores the Fontys
password.** The student types it into Fontys's own page (Microsoft SSO, with MFA); Ledger only picks up the Portflow
session that results.

1. **Settings → Portflow → Sign in with Fontys** opens a Tauri `WebviewWindow` on the Canvas launch URL. It has its own
   persistent data directory in the app's data folder (not the vault, never synced), so the Canvas and Microsoft cookies
   survive restarts like a browser profile.
2. The student signs in. Canvas runs the LTI launch inside the window.
3. An initialization script that runs in every frame (`initialization_script_for_all_frames`), and only acts on
   `https://portfolio.drieam.app`, reads `document.body.dataset.session` and the CSRF meta tag and hands them to Rust.
   How it gets them across is the one technical choice to prove in phase 0 (in order of preference):
   - a Tauri command allowed only for the remote origin `https://portfolio.drieam.app` (capability `remote.urls`), with
     nothing else exposed to that origin;
   - or a navigation to a sentinel URL that the window's navigation handler catches and cancels.
4. The window closes. Rust keeps the token **in memory only** (it lives 8 hours; there is nothing to gain from writing
   it down). It knows the expiry from the JWT's `exp`.
5. **Refreshing**: shortly before `exp`, or on any 401, Ledger repeats the launch in a hidden window. While the Canvas
   session lasts that needs no input. When it has ended, the sync pauses and Ledger shows "Sign in to Portflow again"
   (like the GitHub sync shows a bad token), and the student signs in again in the visible window.

Rejected:

- **Storing the Fontys username and password and scripting the login**: the password would sit on disk, MFA breaks it,
  every change to the login page breaks it, and it is the kind of automation an institution's acceptable-use rules
  forbid.
- **The mobile app's QR pairing** (`users/current/mobile-login`, `mobile/attempts/{id}/confirm`): it would mean
  impersonating Drieam's own mobile client, whose protocol isn't public.
- **`/api/v1/oauth2/*`**: this is Portflow authorising itself against the LMS (to import submissions), not an OAuth
  server for other clients.
- **A Canvas access token with the sessionless launch API**: worth a look in phase 0 as a fallback only; Fontys may not
  let students create tokens, and LTI 1.3 still needs the browser for the launch.

In parallel: **ask Fontys ICT / Drieam whether there is sanctioned API access** for a student's own portfolio. If there
is, it replaces section 2 and nothing else changes, because everything else talks to a `Portflow` trait.

---

## 3. Off, read only, or read and write: your choice

### Switched off: as if it isn't there

Portflow is **off by default**. Settings → Portflow then shows one thing: the switch, with a line on what the feature
does. Switched off means fully off, in the UI and in what runs:

- **UI**: no Outcomes page, no evidence cards in threads, no "Deliver" in the publish dialog, no Portflow section on the
  project overview or beside a deliverable, no person picker fed from Portflow (the "from" field takes a typed name), no
  Portflow card on the dashboard, no Portflow status in the explorer's footer, no Portflow results in search. The
  screens look exactly as they do today. The frontend asks one flag (`portflow.enabled`) and renders none of it.
- **Backend**: the sync worker isn't started, no login window is opened, no request goes to Portflow or Canvas, and
  every Portflow and evidence command answers "Portflow is switched off". The access gate in `portflow_api.rs`
  refuses everything, so even a bug can't reach the network.
- **The vault**: Ledger doesn't read the Portflow files (`portflow/`, `projects/<p>/evidence/`,
  `projects/<p>/portflow/`), and doesn't write, repair or delete them. They are files Ledger doesn't know about, which
  it leaves alone, and the GitHub sync carries them like any other file. The Portflow fields in other files
  (`portflowCollections` in a project, `from` on feedback, …) are kept as they are when Ledger rewrites those files.
  Feedback keeps its "from" name, shown as plain text.
- **Publishing** works as today, with no majors reserved. Switching back on checks the numbers: a major you made while
  it was off that isn't in Portflow is shown, and nothing is written until you have decided what to do with it.
- **Switching off** stops the worker, drops the token from memory and closes any Portflow window. **Sign out and forget**
  (also offered when switching off) deletes the login window's cookies and data folder too, so the next switch-on
  starts with a fresh Fontys login.
- The switch is kept in `.ledger/portflow.json`, so it is per machine and never synced: switching it on at home doesn't
  switch it on at school.
- Tests: with the switch off, a full app round (start, open screens, publish, sync to GitHub) makes no request to the
  fake Portflow, and leaves the Portflow files in the vault byte for byte as they were.

### Switched on: read only, or read and write

**Settings → Portflow → Access** has two settings:

- **Read only** (the default, and what you get after signing in): Ledger reads collections, outcomes, evidence,
  versions, feedback and the people with access, and never changes anything in Portflow.
- **Read and write**: Ledger may also write to Portflow. Each kind of write has its own switch, all off until you
  turn them on:

  | Write | What it does in Portflow |
  |---|---|
  | Upload versions | a new version of an evidence item when you publish (also asked in the publish dialog) |
  | Create evidence | a new evidence item for a deliverable that has none |
  | Edit collection descriptions | your edits to the semester's collection description |
  | Link outcomes | which sub-goals an evidence item counts for |
  | Reply to feedback | your reply under a coach's comment |
  | Ask for feedback | a feedback request on a version, to someone with access |
  | Ask to confirm feedback | a comment with feedback you got in person, and a request to the teacher to confirm it |

How it holds:

- **Enforced in Rust, not just in the UI.** Every request goes through one gate in `portflow_api.rs`. In read-only mode
  the gate lets only `GET` through (plus the one read that Portflow makes with a `POST`, resolving mentions). Anything
  else fails with "Portflow is read-only", whatever asks for it, the sync worker included. Each write kind is checked
  the same way. Tests cover it: in read-only mode no non-GET request ever reaches the fake Portflow.
- **Nothing piles up behind your back.** In read-only mode a Ledger edit that would have gone to Portflow (an edited
  collection description, a published version of a linked deliverable) is kept in Ledger and marked "not in Portflow".
  When you switch writing on, Ledger lists what it would send and sends nothing until you confirm, per item or all at
  once.
- **Every write is visible**: the sync report says what was written ("uploaded v6 of Project plan"), and the app log
  records it.
- The choice is kept in `.ledger/portflow.json`, so each machine decides for itself.

---

## 4. Feedback you got from someone: tag the person

Not all feedback is written in Portflow: much of it is given in a meeting, at a demo or in the hallway. In Ledger you
note it as feedback, as today, and say **who gave it**:

- **The picker** offers the people your portfolio (or that collection or evidence item) is shared with in Portflow,
  teachers first, then the others, with their role. It is refreshed on every sync. Shares that ran out are still shown
  for older feedback but no longer offered for new feedback. Someone who isn't in Portflow can be typed as a name.
- **What is kept**: the feedback file gets `from` (the person's name), `fromPortflowUser` (their Portflow id) and
  `fromRole` (teacher, student, …), plus how it was given (`given: in person | portflow | email | other`) and when.
  Feedback pulled from Portflow fills these in itself, from the comment's author.
- **What it gives you**: per person, all the feedback they gave you; per learning outcome, who gave feedback on it. The
  feedback card shows the person, and the Outcomes page and the deliverable's Produced by window show it too.
- **With "Ask to confirm feedback" on** (read and write), Ledger can also post that feedback to Portflow on the evidence
  version, as your comment ("Feedback from <teacher> on 12-11-2026: …"), and ask that teacher to confirm it through
  Portflow's comment validation. When they confirm (or decline), the next sync shows it on the feedback in Ledger.
  The body of that request is still to be captured (phase 0), like the upload's.

---

## 5. How Portflow maps onto Ledger

| Portflow | Ledger | Direction |
|---|---|---|
| Portfolio | the vault's Portflow link (`.ledger/portflow.json`: host, tool, portfolio id) | |
| Section | a semester (`semesters/<id>.md` gets `portflowSection`) | link |
| Goal set | that semester's outcomes (`semesters/<id>.md` gets `portflowGoalSet`) | ← |
| Goal (learning outcome) + sub-goals | `portflow/outcomes/<goal-id>-<slug>.md`, read-only mirror | ← always |
| Collection | a project **in one semester**: `portflowCollections: {<semester-id>: <collection-id>}` in `project.md` | link |
| Collection description | `projects/<p>/portflow/<semester-id>.md`, with `@`-mentions as links to evidence | ⇄ |
| Collection goals | the outcomes of the project in that semester, in the same file | ← |
| Evidence | **an evidence item**, new in Ledger: `projects/<p>/evidence/<slug>.md`, one or more deliverables (files) and one thread | ⇄ |
| Evidence version N | **major version N** of the evidence's files (`name@vN.0.ext`); Ledger's minors in between stay in Ledger | ⇄ |
| Evidence ↔ goals, sub-goals | `outcomes:` in the evidence item, picked from the mirror | ⇄ links only |
| Comment (root) | feedback in the evidence's thread, on the evidence at the version it names | ← (reply →) |
| Comment children | posts in that thread replying to (quoting) the feedback | ← (reply →) |
| Collection comments | posts in the project's "Portflow: <collection>" thread | ← |
| Feedback request | a prompt-like card: whom you asked, answered or not | ⇄ |
| People with access (shares) | the people to tag on feedback, `portflow/people.md` | ← |
| Comment validation | "confirmed by <teacher>" on feedback you noted yourself | ⇄ |

### Evidence items: files and threads

An evidence item is what you hand in: **one or more files** (a PDF with the document, a ZIP with the sources, …),
delivered together, versioned together, and judged against the same learning outcomes. Ledger gets it as a concept of
its own, because a deliverable in Ledger is one file and a Portflow version can hold several.

```
thread "Project plan"  ──┬── evidence "Project plan PoliceSpeedEdit"   files: projectplan.pdf
                         │                                              outcomes: Analysis-U1, Analysis-I1, Analysis-S1
                         └── evidence "Research questions"             files: research.pdf, interviews.zip
                                                                        outcomes: Advise-…, PS
```

- **An evidence item belongs to one thread**: that is where its feedback lands, and where you work on it. **A thread
  can hold several evidence items**: one line of work often delivers more than one thing. So it is one-to-many, not
  one-to-one.
- **A file belongs to at most one evidence item.** A deliverable in no evidence item is just a deliverable, as today.
- **An evidence item can exist before Portflow knows it** (no `portflowEvidence` yet). You can give it files and link
  learning outcomes in Ledger, also in read-only mode; it goes to Portflow when you deliver it (or when you link it to
  one you created in Portflow yourself).
- **Feedback lands on the evidence item at a version** ("on v5"), not on a single file, because that is what a coach
  sees in Portflow. It can still point at one of the files when it is about that file.

```yaml
# projects/<p>/evidence/project-plan.md
---
name: Project plan PoliceSpeedEdit
thread: project-plan
files: [projectplan.md, projectplan-sources.zip]   # deliverables, by their path in deliverables/
outcomes: [494334, 96390, 96396, 96399]            # goal and sub-goal ids from portflow/outcomes/
collections: {nj26-s3: 261038}                     # where it is shown, per semester
portflowEvidence: 2110705                          # empty until it exists in Portflow
type: file                                         # file | free_text | url | video | lms_submission | badge
versions:                                          # Portflow version = Ledger major, per file
  1: {portflowVersion: 3074813, type: free_text}
  5: {portflowVersion: 3173444, files: {projectplan.md: projectplan@v5.0.pdf, projectplan-sources.zip: projectplan-sources@v5.0.zip}}
---
What this evidence shows, in your words (the evidence's description in Portflow).
```

### Versions: majors are Portflow's, minors are Ledger's

**Rule: version N in Portflow is major N in Ledger, for every file of the evidence item.** While you work you publish
minors in Ledger (v5.1, v5.2, … each with its changelog, its Produced by and its AI-use label, as today). Portflow
never sees a minor. Delivering is what makes the next major, and the major number is always Portflow's next version
number, so the two can't drift.

- **Every delivery bumps the whole evidence item.** Deliver (on the evidence card in the thread, or "Publish and
  deliver" in the publish dialog) publishes v(N+1).0 of **every** file in the evidence item and uploads them all as
  Portflow version N+1, in one go. Deliver the document and the ZIP and both are v2; deliver again and both are v3,
  also the one that didn't change (it gets the same bytes under the new number). So all files of an evidence item
  always have the same majors, equal to Portflow's versions, and every Portflow version holds the complete set.
- **The description that goes with it** starts as the changelogs of the minors since vN.0 ("v5.1: …, v5.2: …"), which
  you edit, plus "Made with AI: <MMM label>" when the version is marked as made with AI. It becomes the major's
  changelog in Ledger too.
- **The publish dialog guards the rule.** For a file in a linked evidence item, "major" and a custom version whose major
  isn't the current one are replaced by Deliver; a minor is always allowed.
- **A version made in Portflow** (uploaded in the browser) comes in as major N+1 of the evidence's files, with
  Portflow's description as its changelog and `producedBy: portflow`. Ledger's minors on top of vN stay where they are
  (they are drafts after vN). The live file only takes the new version when it has no unpublished edits; otherwise
  Ledger keeps your edits, puts Portflow's file in `.ledger/conflicts/`, and says so.
- **Read only** can't upload, so Deliver says where to upload it in Portflow (it opens the evidence there and shows the
  files in your file manager). The next sync finds Portflow's new version, checks it against your latest minors by
  checksum, and records it as major N+1: matching files get vN+1.0 as the same bytes, others come in as Portflow's.
- **Linking an existing evidence item** (like the project plan with five versions) imports Portflow's versions under
  their own numbers (v1.0 … v5.0), downloading the files. Version 1 here is a `free_text` placeholder with no file, and
  is recorded but has no file in Ledger. If your Ledger files already have majors, Ledger compares them by checksum
  first: matches are linked, and anything that doesn't line up is shown to you before anything is written.
- **v0.x** is "not yet delivered": a new evidence item's files publish minors on v0 until the first delivery makes v1.0.
- **Version types**: `file` → the files themselves; `free_text` → a markdown file in the evidence; `url` → a markdown file
  with the link; `video`, `lms_submission`, `badge` → recorded, opened in Portflow.

### Where the learning outcomes are kept: in the vault

The outcomes are linked to evidence and so to deliverables, so **they live in the vault**, like everything else
Ledger links:

- **The vault is Ledger's source of truth; the in-memory database is built from it** at every start and is never kept
  on its own. An outcome only in the database would be gone after a restart, would not reach the other machine, and
  Claude Code (which reads the vault) couldn't see which outcomes a deliverable is for.
- So the mirror is files, `portflow/outcomes/<goal-id>-<slug>.md`, synced to GitHub with the rest. The links are in
  the evidence items (`outcomes:`). With both in the vault, links work offline, on every machine, and in a run.
- At startup they are read into new tables in `schema.sql` (`outcomes`, `sub_outcomes`, `evidence`, `evidence_files`,
  `evidence_outcomes`, `evidence_versions`), so the Outcomes page and "which evidence counts for Analysis-S1?" are
  quick queries, the same as the rest of Ledger.
- Only what is about talking to Portflow stays out of the vault, in `.ledger/portflow-sync.json`: etags, the last
  agreed hashes, the time of the last refresh.
- Portflow stays the owner: every sync rewrites the mirror, and an edit to a mirrored file is refused in the app and
  overwritten by the next sync (the file says so at the top).

### The vault, added

```
portflow/outcomes/<goal-id>-<slug>.md                learning outcome: name, nickname, goal set, status, explanation,
                                                     and its sub-goals (id, name, description) as sections. Mirrored
                                                     from Portflow on every sync; read-only.
portflow/people.md                                   the people your portfolio is shared with: Portflow id, name, role,
                                                     what is shared with them, until when. Mirrored from Portflow.
semesters/<id>.md                                    + portflowSection, portflowGoalSet
projects/<p>/project.md                              + portflowCollections: one collection per semester
projects/<p>/portflow/<semester-id>.md               that semester's collection: subtitle, outcomes, and the
                                                     description as markdown
projects/<p>/evidence/<slug>.md                      an evidence item: files, thread, outcomes, collections, the
                                                     Portflow id, and Portflow version ↔ Ledger major per file
projects/<p>/threads/<t>/thread.md                   (unchanged: the thread's evidence items point at it)
projects/<p>/versions/<id>.md                        + evidence, portflowVersion (on majors that went to / came from Portflow)
projects/<p>/threads/<t>/feedback/<id>.md            + evidence and evidenceVersion (feedback on an evidence item),
                                                       from, fromPortflowUser, fromRole, given (in person | portflow | …),
                                                       and for feedback from Portflow: source: portflow, portflowComment,
                                                       commentType, confirmed (by whom, when)
.ledger/portflow.json                                host, tool id, portfolio id, access (read only, or read and write
                                                     and which writes)                                 (never synced)
.ledger/portflow-sync.json                           what both sides last agreed on: hashes, etags, ids     (never synced)
```

Coaches' names land in the vault, which is fine for a private repository and covered by the vault's encryption when
it is switched on.

### Rules

- **One collection per project per semester.** Linking a project picks a collection for one semester; a project that
  carries on gets the next semester's collection linked next to it, and the old link stays (that semester's description,
  outcomes and feedback remain readable). The project overview shows the current semester's collection. The semester
  comes from the collection's section, matched to a Ledger semester by its dates and name. A section with no matching
  Ledger semester offers to create one.
- **Learning outcomes are Portflow's.** Ledger never creates, renames or rewrites a goal or sub-goal. An outcome that
  disappeared from Portflow is marked `removed` (not deleted, so links to it keep a name). Ledger may change only which
  evidence is linked to which goal and sub-goal, because that is the student's own choice in Portflow too.
- **Feedback from others is read-only in Ledger.** It can be replied to (that goes to Portflow as a child comment) and
  marked resolved (that stays in Ledger: Portflow has no such state).
- **Versions are append-only on both sides**, and majors match (above).
- **The collection description is three-way merged** like Trello cards: base in `portflow-sync.json`, the side that
  changed wins, both changed → this machine wins and the other goes to `.ledger/conflicts/`.
- **HTML ⇄ markdown**: Portflow stores HTML (TipTap). Ledger converts to markdown on the way in and back to HTML on the
  way out (pulldown-cmark is already there; add an HTML-to-markdown crate). A mention becomes
  `[@Project plan](../evidence/project-plan.md "portflow:evidence:2110705")` and turns back into the same span. A
  round trip of unchanged text must give the same HTML, or the sync would push on every round; this is tested.

---

## 6. Code: same shape as the Trello sync

```
src-tauri/src/
  portflow_api.rs      the Portflow trait and its reqwest implementation: typed, lenient models (#[serde(default)],
                       unknown fields ignored), pagination, etags (If-None-Match), 401 → SessionExpired, polite pacing,
                       and the access gate every request passes (read only / which writes are allowed)
  portflow_auth.rs     the sign-in window, picking up the session, the hidden refresh, the token in memory
  portflow_config.rs   .ledger/portflow.json and .ledger/portflow-sync.json
  portflow_html.rs     HTML ⇄ markdown, mentions ⇄ links
  evidence.rs          evidence items: files, thread, outcomes, the major ↔ Portflow version map; Deliver; a store like
                       the others (works signed out and offline; off with the feature)
  outcomes.rs          the outcomes mirror: read from portflow/outcomes/, queried for the Outcomes page
  schema.sql           + outcomes, sub_outcomes, evidence, evidence_files, evidence_outcomes, evidence_versions
  vault.rs             + the new files (lenient import, canonical export), the mirror refused for edits
  portflow_ledger.rs   the vault side, behind a trait (like trello_ledger.rs), never holding the vault lock over a request
  portflow_sync.rs     the reconcile: a pure function from (Portflow state, vault state, base) to actions and a report
  portflow.rs          Tauri commands and the background worker: after vault changes (debounced), every ~10 minutes
                       (cheap check first: notifications unread-count and etags), and on demand
  tests_portflow.rs    against a fake Portflow, with fixtures from real responses (scrubbed of names and ids)
src/lib/
  components/settings/PortflowSettings.svelte    sign in / out, status, portfolio, access (read only / read and
                                                 write, with a switch per write), link projects to collections
  components/PersonPicker.svelte                 who gave the feedback: the people with access, or a typed name
  routes/outcomes/                               learning outcomes: per goal its sub-goals, status, linked
                                                 deliverables and feedback; filter by semester (goal set)
  components/EvidenceCard.svelte                 in a thread: the evidence item's files, outcomes, versions (majors
                                                 with their Portflow status, minors since), Deliver, Ask for feedback
  FeedbackCard (existing)                        who gave it, role, "from Portflow", on which evidence and version, reply box
  publish dialog (existing)                      minors as today; "Publish and deliver" instead of major for evidence files
  deliverable side window (existing)             its evidence item, and its outcomes
  project overview (existing)                    the collection's outcomes and description
```

The sync worker reports through events like the other two and pauses (doesn't fail) when signed out. The GitHub sync
then pushes whatever a Portflow round wrote, and the two can't echo each other for the same reason the Trello sync
can't: a round writes what Portflow says, and a round that finds both sides agreeing writes nothing.

Commit scope: **`portflow`** (in the scopes table in CONTRIBUTING.md and its copy in CLAUDE.md). Evidence items and the
outcomes mirror are part of the Portflow feature, so they use it too.

---

## 7. Phases

### Phase 0: prove the risky parts (no Ledger features yet)

- [ ] Sign-in window on Linux, Windows and macOS: Fontys login, LTI launch, session picked up; pick the hand-over
      mechanism (remote-origin command vs navigation sentinel).
- [ ] Silent refresh in a hidden window after the token expires; what happens when the Canvas session is gone.
- [ ] Is `X-CSRF-Token` needed next to the Bearer token, for reads and for writes?
- [ ] Capture one real "new version" upload in the browser (with go-ahead, on a test evidence item) to pin the body of
      `POST …/evidence/{e}/versions` and `POST …/evidence`; the same for a comment validation request. With two files
      in it: must a new version carry every file again, or can it reuse the previous version's (signed_id)?
- [ ] Read Portflow's acceptable-use policy (drieam.com/portflow/aup) and ask Fontys ICT whether this is welcome / whether
      there is sanctioned API access.

### Phase 1: read everything into Ledger (pull only)

- [ ] The on/off switch first, off by default: the `portflow.enabled` flag, the frontend gating everything on it, the
      backend refusing and not starting, the vault not reading the Portflow files; the "off makes no request and
      changes no file" test.
- [ ] `portflow_api.rs` reads + fixtures, with the access gate (read only is the only mode in this phase);
      `portflow_auth.rs`; settings screen with sign-in, status and access.
- [ ] People with access (`portflow/people.md`); the person picker on feedback; `from` / `fromRole` / `given` on
      feedback, filled in for feedback pulled from Portflow.
- [ ] Sections ↔ semesters (match or create); learning outcomes mirror (`portflow/outcomes/`), goal set per semester,
      the Outcomes page.
- [ ] Link a project to a collection per semester; mirror each one's description (read-only in this phase) and its
      outcomes.
- [ ] Evidence items in Ledger (`evidence.rs`, the vault file, the tables, the card in a thread): files, thread,
      outcomes. Works signed out and offline.
- [ ] Link Portflow evidence to evidence items (match by name, confirm by hand), in the thread you pick; pull its
      versions as majors (download the files while the signed URL is fresh), checksums against existing majors.
- [ ] The publish dialog's rule: minors only for files of a linked evidence item; Deliver in read-only mode (upload by
      hand, matched on the next sync).
- [ ] Comments → feedback on the evidence item at its version, replies as posts, in the evidence's thread; new
      feedback shown on the dashboard.

### Phase 2: deliverables to Portflow

- [ ] Read and write mode with its switches; the list of what is waiting to be sent, confirmed before anything goes.
- [ ] Direct upload (MD5, `direct-uploads`, PUT to S3) of every file, then Portflow version N+1 = Ledger major N+1, with
      the minors' changelogs + AI label as the description.
- [ ] Create the Portflow evidence for an evidence item that has none, in the semester's collection, linked to its
      outcomes.

### Phase 3: two-way

- [ ] Collection description editable in Ledger, three-way merged, mentions ⇄ links.
- [ ] Outcome links made in Ledger go to Portflow (and Portflow's come back), three-way like the description.
- [ ] Reply to feedback from Ledger; ask for feedback on a version (pick from the people the evidence is shared with).
- [ ] Feedback you got in person: post it to Portflow and ask the teacher to confirm it (comment validation); show
      confirmed or declined on the feedback in Ledger.

### Phase 4: polish

- [ ] Notifications drive the polling; a Portflow card on the dashboard (like the GitHub sync card).
- [ ] Claude Code runs get the linked learning outcomes and open feedback as context for a prompt.
- [ ] README: the Portflow section, the vault additions, what is never synced.

---

## 8. Risks

| Risk | What we do |
|---|---|
| The API is internal and undocumented; Drieam can change it any day | Lenient parsing; contract tests on fixtures; a change shows up as "Portflow changed; sync paused" instead of corrupt files; never delete in the vault because of a missing field. |
| Using the student's own session by program may be against the AUP | Read it first (phase 0), ask Fontys; keep traffic low and human-paced; only the student's own portfolio. |
| The token lives 8 hours; the Canvas session ends sooner or later | Silent refresh; a clear "sign in again" state; the sync pauses, nothing is lost. |
| Signed S3 URLs expire after 30 minutes | Ask for the preview/download URL right before downloading. |
| HTML round trips change text and cause endless pushes | Normalise before comparing; round-trip tests on real descriptions. |
| A bug or a sync round writes to Portflow when it shouldn't | Read only is the default; one gate checks every request against the access setting; tests prove that read only sends no write. |
| A version uploaded twice (retry after a timeout) | Before uploading, list the versions and match by file checksum and description; record the Portflow version id in `versions/<id>.md` as soon as it exists. |
| The coaches' comments are personal data | They stay in the student's private vault and repository (encrypted when that is on); never logged in full in the app log. |

## 9. Open questions for you

Settled:

- Portflow is off by default and fully off when switched off, in the UI and in what runs.
- An evidence item has one thread; a thread can have several evidence items.
- An evidence item has one or more files; a file belongs to one evidence item only.
- Every delivery bumps all files of the evidence item to the next major, equal to the Portflow version; minors stay in
  Ledger. Versions made in Portflow come in automatically as majors.
- The outcomes and their links live in the vault, and are loaded into the in-memory database like everything else.

Nothing open right now; phase 0 will bring the next questions (the upload body, whether files must be uploaded again).
