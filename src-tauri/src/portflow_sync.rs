//! One Portflow round: read what Portflow has and bring the vault in step with it. Pull only, but for one write the
//! person can switch on: a study project's reading guide is the description of the collection it is linked to for the
//! current semester, kept in step both ways (see [`sync_guide`]); it goes back to Portflow only with that write on.
//! Every other write would be refused by the access gate in `portflow_api.rs`.
//!
//! A round reads, in this order: who is signed in; the learning outcomes with their sub-goals and goal sets; the people
//! the portfolio is shared with; the sections (linked to semesters) and collections; and for every collection linked to a project, its
//! outcomes and its evidence. Per evidence item it reads the outcomes it counts for, its versions, and, once the item is
//! in a thread, its comments, which become feedback there.
//!
//! Files are never downloaded. A version whose files Ledger published itself (the same name at major N, `plan@v2.0.pdf`)
//! is linked to those deliverables; a version uploaded in Portflow by hand is recorded by name only, and the screens
//! open it in Portflow. So Portflow can't fill the vault with copies of what lives there.
//!
//! The vault is never locked while Portflow is asked something: a round reads, then writes what it read in one short
//! step per part, through [`Ledger`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::portflow_api::{Comment, PResult, Portflow, PortflowError};
use crate::portflow_config::{GuideMemory, GuideSeen};
use crate::portflow_config::PortflowSettings;
use crate::portflow_store::{self as store, CollectionLink, VersionFileRow};
use crate::state::Core;


/// The vault, as a round sees it.
pub trait Ledger: Send + Sync {
    fn read(&self, f: &mut dyn FnMut(&Connection) -> Result<()>) -> Result<()>;
    /// Changes the database and writes the difference to the vault. Returns how many files that touched.
    fn write(&self, f: &mut dyn FnMut(&Connection, &Path) -> Result<()>) -> Result<usize>;
    /// The vault's folder: its `.ledger/` holds this machine's bookkeeping (see `portflow_config::GuideMemory`).
    fn root(&self) -> PathBuf;
}

/// The running app's vault. `changed` tells the GitHub sync there is something to push.
pub struct CoreLedger {
    pub core: Arc<Mutex<Core>>,
    pub changed: Arc<dyn Fn() + Send + Sync>,
}

impl Ledger for CoreLedger {
    fn read(&self, f: &mut dyn FnMut(&Connection) -> Result<()>) -> Result<()> {
        let core = self.core.lock().unwrap_or_else(PoisonError::into_inner);
        f(&core.conn)
    }

    fn write(&self, f: &mut dyn FnMut(&Connection, &Path) -> Result<()>) -> Result<usize> {
        let touched = {
            let mut guard = self.core.lock().unwrap_or_else(PoisonError::into_inner);
            let core = &mut *guard;
            let tx = core.conn.unchecked_transaction()?;
            f(&tx, core.vault.root())?;
            tx.commit()?;
            core.vault.persist(&core.conn)?
        };
        if touched > 0 {
            (self.changed)();
        }
        Ok(touched)
    }

    fn root(&self) -> PathBuf {
        self.core.lock().unwrap_or_else(PoisonError::into_inner).vault.root().to_path_buf()
    }
}

/// What a round did.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub outcomes: usize,
    pub people: usize,
    pub collections: usize,
    pub evidence: usize,
    /// Versions recorded this round, and how many of their files are in Portflow only (uploaded there; not copied:
    /// a project that isn't a study one, or a download that didn't work yet).
    pub new_versions: usize,
    pub in_portflow_only: usize,
    /// Files downloaded from Portflow this round, each now a published version of a deliverable.
    pub downloaded: usize,
    /// Deliverables renamed this round to follow their evidence item's name.
    pub renamed: usize,
    /// Comments from others that arrived as feedback this round.
    pub new_feedback: usize,
    /// Evidence items that are in no thread yet, so their feedback waits.
    pub unplaced: usize,
    /// Files in the vault this round wrote or removed.
    pub files_touched: usize,
    /// Reading guides written to a collection's description this round.
    pub guides_written: usize,
    /// Reading guides taken from a collection's description this round (changed in Portflow).
    pub guides_read: usize,
    /// Evidence descriptions taken from Portflow, and written to it, this round (`portflow_evidence_text`).
    pub descriptions_read: usize,
    pub descriptions_written: usize,
    /// Things a person should know, such as a linked collection that is gone.
    pub notes: Vec<String>,
}

impl Report {
    pub fn vault_changed(&self) -> bool {
        self.files_touched > 0
    }

    pub fn summary(&self) -> String {
        let mut parts = vec![];
        if self.new_feedback > 0 {
            parts.push(format!("{} new feedback", self.new_feedback));
        }
        if self.new_versions > 0 {
            parts.push(format!("{} new version{}", self.new_versions, if self.new_versions == 1 { "" } else { "s" }));
        }
        if self.downloaded > 0 {
            parts.push(format!("{} file{} downloaded as deliverables", self.downloaded, if self.downloaded == 1 { "" } else { "s" }));
        }
        if self.renamed > 0 {
            parts.push(format!("{} deliverable{} renamed after {} evidence item", self.renamed, if self.renamed == 1 { "" } else { "s" }, if self.renamed == 1 { "its" } else { "their" }));
        }
        if self.unplaced > 0 {
            parts.push(format!("{} evidence item{} to put in a thread", self.unplaced, if self.unplaced == 1 { "" } else { "s" }));
        }
        if self.guides_read > 0 {
            parts.push(format!("{} reading guide{} taken from Portflow", self.guides_read, if self.guides_read == 1 { "" } else { "s" }));
        }
        if self.guides_written > 0 {
            parts.push(format!("{} reading guide{} written to Portflow", self.guides_written, if self.guides_written == 1 { "" } else { "s" }));
        }
        if self.descriptions_read > 0 {
            parts.push(format!("{} evidence description{} taken from Portflow", self.descriptions_read, if self.descriptions_read == 1 { "" } else { "s" }));
        }
        if self.descriptions_written > 0 {
            parts.push(format!("{} evidence description{} written to Portflow", self.descriptions_written, if self.descriptions_written == 1 { "" } else { "s" }));
        }
        if parts.is_empty() {
            parts.push("Up to date".into());
        }
        format!(
            "{}. {} outcome{}, {} linked collection{}, {} evidence item{}.",
            parts.join(", "),
            self.outcomes,
            if self.outcomes == 1 { "" } else { "s" },
            self.collections,
            if self.collections == 1 { "" } else { "s" },
            self.evidence,
            if self.evidence == 1 { "" } else { "s" }
        )
    }
}

fn ledger_err(e: Error) -> PortflowError {
    PortflowError::Ledger(e)
}

/// The portfolio a round reads: the one found at sign-in.
fn portfolio_of(settings: &PortflowSettings) -> PResult<i64> {
    settings.portfolio.as_ref().map(|p| p.id).filter(|id| *id > 0).ok_or_else(|| PortflowError::message("Sign in to Portflow first, so Ledger knows which portfolio is yours."))
}

pub fn run(portflow: &dyn Portflow, ledger: &dyn Ledger, settings: &PortflowSettings) -> PResult<Report> {
    if !settings.enabled {
        return Err(PortflowError::Off);
    }
    let p = portfolio_of(settings)?;
    let mut report = Report::default();
    let me = portflow.current_user()?;

    // Learning outcomes, always from Portflow.
    let sets = portflow.goal_sets(p)?;
    let goals = portflow.goals(p)?;
    let mut with_subs = Vec::with_capacity(goals.len());
    for goal in goals {
        let subs = portflow.sub_goals(p, goal.id)?;
        with_subs.push((goal, subs));
    }
    report.outcomes = with_subs.len();
    let shares = portflow.shares(p)?;
    // The sections are the portfolio's semesters: mirrored, and a new one linked to the semester it falls in.
    let section_list = portflow.sections(p)?;
    report.files_touched += ledger
        .write(&mut |conn, _| {
            store::store_outcomes(conn, &with_subs, &sets)?;
            store::store_people(conn, &shares)?;
            crate::portflow_epsilon::store_sections(conn, &section_list)
        })
        .map_err(ledger_err)?;
    report.people = shares.iter().map(|s| s.invitee.id).collect::<HashSet<_>>().len();

    // The linked collections.
    let sections: HashMap<i64, String> = section_list.into_iter().map(|s| (s.id, s.name)).collect();
    let collections: HashMap<i64, crate::portflow_api::Collection> = portflow.collections(p)?.into_iter().map(|c| (c.id, c)).collect();
    let mut links = vec![];
    // The collection whose description each study project's reading guide is (see `sync_guide`): always read in, and
    // written back only with that write on.
    let guide_on = settings.access.allows(crate::portflow_config::GUIDE_WRITE);
    let mut guides: HashMap<i64, store::GuideTarget> = HashMap::new();
    // The projects whose evidence comes in as deliverables (see `deliver_evidence`).
    let mut delivering = std::collections::BTreeSet::new();
    let today = crate::ids::now().chars().take(10).collect::<String>();
    ledger
        .read(&mut |conn| {
            links = store::collection_links(conn)?;
            guides = store::guide_targets(conn, &today)?.into_iter().map(|g| (g.collection, g)).collect();
            delivering = store::delivering_projects(conn)?;
            Ok(())
        })
        .map_err(ledger_err)?;
    let root = ledger.root();
    let mut memory = crate::portflow_config::read_guides(&root);
    // The projects whose evidence descriptions are kept in step: the same ones as the reading guides (linked, a study
    // project or one without a kind yet), each with the "Evidence description" write for the way back.
    let described: std::collections::HashSet<String> = guides.values().map(|g| g.project.clone()).collect();
    let described_on = settings.access.allows(crate::portflow_config::EVIDENCE_WRITE);

    for link in &links {
        let Some(found) = collections.get(&link.collection) else {
            report.notes.push(format!("The collection linked to {} for {} isn't in your portfolio any more.", link.project, link.semester));
            continue;
        };
        let mut collection = found.clone();
        if let Some(guide) = guides.get(&link.collection).filter(|g| g.project == link.project) {
            sync_guide(portflow, ledger, p, guide, &mut collection, guide_on, &mut memory, &root, &mut report)?;
        }
        let collection = &collection;
        report.collections += 1;
        let goals = portflow.collection_goals(p, link.collection)?;
        let section = collection.section_id.and_then(|s| sections.get(&s)).cloned().unwrap_or_default();
        report.files_touched += ledger.write(&mut |conn, _| store::store_collection(conn, link, collection, &section, &goals)).map_err(ledger_err)?;

        for item in portflow.collection_evidence(p, link.collection)? {
            report.evidence += 1;
            pull_evidence(portflow, ledger, p, me.id, link, &item.evidence, delivering.contains(&link.project), &mut report)?;
            if described.contains(&link.project) {
                crate::portflow_evidence_text::round(portflow, ledger, p, &link.project, &item.evidence, described_on, &mut memory, &root, &mut report)?;
            }
        }
    }
    Ok(report)
}

/// Keeps a project's reading guide and its collection's description one text: README.md in Ledger, the description in
/// Portflow. Which side changed since they last agreed is told by what is remembered (`GuideMemory`, in `.ledger/`):
///
/// - only Portflow's: the description, as markdown, becomes the README (an emptied description doesn't empty it);
/// - only Ledger's: with the guide write on, the README goes to Portflow (Ledger's text wins); off, it waits in Ledger;
/// - both: Ledger's text is kept, and sent with the write on; with it off the report says both changed;
/// - neither, or both say the same: nothing.
///
/// The first time, with nothing remembered, the description comes in; a README that said something else is set aside in
/// `.ledger/conflicts/` first. An empty guide never empties a description. A refused or failed write is a note, not the
/// end of the round.
#[allow(clippy::too_many_arguments)]
fn sync_guide(
    portflow: &dyn Portflow,
    ledger: &dyn Ledger,
    p: i64,
    guide: &store::GuideTarget,
    collection: &mut crate::portflow_api::Collection,
    write_on: bool,
    memory: &mut GuideMemory,
    root: &Path,
    report: &mut Report,
) -> PResult<()> {
    let key = format!("{}/{}", guide.project, collection.id);
    let description = collection.description.clone().unwrap_or_default();
    let readme = guide.readme.clone();
    let now = GuideSeen { description: html_hash(&description), readme: text_hash(&readme) };
    let seen = memory.guides.get(&key).cloned();
    let remember = |memory: &mut GuideMemory, agreed: GuideSeen| -> PResult<()> {
        if memory.guides.get(&key) != Some(&agreed) {
            memory.guides.insert(key.clone(), agreed);
            crate::portflow_config::write_guides(root, memory).map_err(ledger_err)?;
        }
        Ok(())
    };

    // The guide as it would be in Portflow (mentions as spans), and the description as it would be the guide (mentions as
    // tokens): saying the same thing is agreeing, whichever side changed.
    let threads = guide_threads(guide, &description);
    let pulled = crate::portflow_html::to_guide_markdown(&description);
    let blank = |html: &str| crate::portflow_html::to_markdown(html).trim().is_empty();
    if (readme.trim().is_empty() && blank(&description))
        || (!readme.trim().is_empty() && (squash(&pulled) == squash(&readme) || same_text(&crate::portflow_html::to_portflow_html(&readme, &threads), &description)))
    {
        return remember(memory, now);
    }
    // A guide taken in before mentions kept their ids (as bold labels) and not changed since: it takes them now.
    if seen.as_ref().is_some_and(|s| s.description == now.description) && squash(&readme) == squash(&crate::portflow_html::to_markdown(&description)) {
        report.files_touched += ledger.write(&mut |conn, _| crate::projects::save_readme(conn, &guide.project, &pulled).map(|_| ())).map_err(ledger_err)?;
        return remember(memory, GuideSeen { description: now.description, readme: text_hash(&pulled) });
    }
    let (in_portflow, in_ledger) = match &seen {
        Some(s) => (s.description != now.description, s.readme != now.readme),
        None if readme.trim().is_empty() || readme.trim() == crate::projects::starter_readme(&guide.name).trim() => (true, false),
        // The first round for a guide that says something else: the collection's description is what the portfolio
        // shows, so it comes in, and the guide that was there is set aside in .ledger/conflicts/ rather than lost.
        None if !blank(&description) => {
            set_aside(root, &guide.project, &readme).map_err(ledger_err)?;
            report.notes.push(format!(
                "The reading guide of {} now is the description of \"{}\" in Portflow; the guide it had is kept in .ledger/conflicts/.",
                guide.project, collection.name
            ));
            (true, false)
        }
        None => (false, true),
    };

    if in_portflow && !in_ledger {
        let text = pulled.clone();
        if text.trim().is_empty() {
            return remember(memory, now);
        }
        report.files_touched += ledger.write(&mut |conn, _| crate::projects::save_readme(conn, &guide.project, &text).map(|_| ())).map_err(ledger_err)?;
        report.guides_read += 1;
        crate::applog::info("portflow", &format!("Took the reading guide of {} from the description of \"{}\"", guide.project, collection.name));
        return remember(memory, GuideSeen { description: now.description, readme: text_hash(&text) });
    }
    if !in_ledger {
        return Ok(());
    }
    if readme.trim().is_empty() {
        // Emptied in Ledger: the description stays as it is, and a later change there comes back in.
        return remember(memory, now);
    }
    if !write_on {
        if in_portflow {
            report.notes.push(format!(
                "The reading guide of {} and the description of \"{}\" in Portflow both changed: Ledger's guide is kept. Switch on \"Reading guide to collection description\" to send it.",
                guide.project, collection.name
            ));
        }
        // What Portflow has now is seen; Ledger's text stays changed until it is sent.
        let readme = seen.map(|s| s.readme).unwrap_or_default();
        return remember(memory, GuideSeen { description: now.description, readme });
    }
    let html = crate::portflow_html::to_portflow_html(&readme, &threads);
    match portflow.set_collection_description(p, collection.id, &html) {
        Ok(answer) => {
            let stored = answer.unwrap_or_else(|| html.clone());
            report.guides_written += 1;
            crate::applog::info("portflow", &format!("Wrote the reading guide of {} to the description of \"{}\"", guide.project, collection.name));
            remember(memory, GuideSeen { description: html_hash(&stored), readme: now.readme })?;
            collection.description = Some(stored);
        }
        Err(PortflowError::SignedOut) => return Err(PortflowError::SignedOut),
        Err(e) => report.notes.push(format!("The reading guide of {} didn't go to \"{}\": {e}", guide.project, collection.name)),
    }
    Ok(())
}

/// The threads of the guide's project that hold an evidence item, each as a mention of that item, keyed by the thread's
/// slug and its title (folded): a `[[link]]` to one becomes that mention in Portflow. A mention is written with the id
/// form the description already uses (`evidence-2110705`), or that form when it has none.
fn guide_threads(guide: &store::GuideTarget, description: &str) -> std::collections::HashMap<String, crate::portflow_html::Mention> {
    use crate::portflow_html::{Mention, fold, mention_number, to_markdown_with_mentions};
    let (_, existing) = to_markdown_with_mentions(description);
    let mut out = std::collections::HashMap::new();
    for e in &guide.evidence {
        let (Some(slug), Some(title)) = (&e.thread, &e.thread_title) else { continue };
        let id = existing
            .iter()
            .find(|m| m.kind == "evidence" && mention_number(&m.id) == Some(e.portflow_id))
            .map(|m| m.id.clone())
            .unwrap_or_else(|| format!("evidence-{}", e.portflow_id));
        let mention = Mention { kind: "evidence".into(), id, label: e.name.clone() };
        out.entry(fold(slug)).or_insert_with(|| mention.clone());
        out.entry(fold(title)).or_insert(mention);
    }
    out
}

/// Keeps a reading guide that is about to be replaced: `.ledger/conflicts/<time>/portflow/<project>-README.md`, beside
/// what the other syncs set aside. Never synced.
fn set_aside(root: &Path, project: &str, readme: &str) -> crate::error::Result<()> {
    let file = root
        .join(crate::settings::LEDGER_DIR)
        .join("conflicts")
        .join(crate::ids::now().replace(':', "-"))
        .join("portflow")
        .join(format!("{project}-README.md"));
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| crate::error::Error::io(dir, e))?;
    }
    std::fs::write(&file, readme).map_err(|e| crate::error::Error::io(&file, e))
}

/// Makes an evidence item a deliverable, named after the item: each file of its versions that isn't in Ledger yet is
/// downloaded once and stored as published version N.0 (the newest also as the live file, unless that was changed in
/// Ledger), and a deliverable that came from Portflow follows the item's name when it is renamed there. A download that
/// doesn't work is a note, and is tried again next round; signed out ends the round, as anywhere.
fn deliver_evidence(portflow: &dyn Portflow, ledger: &dyn Ledger, p: i64, project: &str, slug: &str, report: &mut Report) -> PResult<()> {
    let mut open = vec![];
    let mut newest = 0;
    ledger
        .read(&mut |conn| {
            open = store::open_files(conn, project, slug)?;
            newest = conn.query_row(
                "SELECT COALESCE(MAX(number), 0) FROM evidence_versions WHERE project_slug = ?1 AND evidence_slug = ?2 AND kind = 'file'",
                [project, slug],
                |r| r.get(0),
            )?;
            Ok(())
        })
        .map_err(ledger_err)?;
    for file in open {
        let bytes = match portflow.file_url(p, file.portflow_file).and_then(|url| portflow.download(&url)) {
            Ok(bytes) => bytes,
            Err(PortflowError::SignedOut) => return Err(PortflowError::SignedOut),
            Err(e) => {
                report.in_portflow_only += 1;
                report.notes.push(format!("\"{}\" (version {}) of {project} didn't download: {e}. Syncing tries again.", file.name, file.number));
                continue;
            }
        };
        let mut kept = None;
        report.files_touched += ledger
            .write(&mut |conn, root| {
                let base = store::base_for_download(conn, project, slug, &file)?;
                if store::store_download(conn, root, project, slug, &file, &base, &bytes, file.number == newest)? == store::Live::Kept {
                    kept = Some(base);
                }
                Ok(())
            })
            .map_err(ledger_err)?;
        report.downloaded += 1;
        if let Some(base) = kept {
            report.notes.push(format!(
                "{project}: {base} was changed in Ledger, so it stays; Portflow's version {} is beside it as v{}.0.",
                file.number, file.number
            ));
        }
    }
    // Asked first, so that a round with nothing to settle writes nothing.
    let mut wanted = false;
    ledger
        .read(&mut |conn| {
            wanted = store::needs_settling(conn, project, slug)?;
            Ok(())
        })
        .map_err(ledger_err)?;
    if wanted {
        let mut moved = vec![];
        report.files_touched += ledger
            .write(&mut |conn, root| {
                moved = store::settle_name(conn, root, project, slug)?;
                Ok(())
            })
            .map_err(ledger_err)?;
        for (from, to) in moved {
            report.renamed += 1;
            crate::applog::info("portflow", format!("Renamed {from} to {to} in {project}, after its evidence item"));
        }
    }
    Ok(())
}

/// A description as Portflow keeps it, white space folded, hashed: what is remembered of it.
fn html_hash(html: &str) -> String {
    crate::provenance::sha256_hex(squash(html).as_bytes())
}

/// A README as Ledger keeps it, hashed.
fn text_hash(text: &str) -> String {
    crate::provenance::sha256_hex(text.replace("\r\n", "\n").trim().as_bytes())
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

/// Whether two bits of HTML hold the same markup once runs of white space are one space.
fn same_text(a: &str, b: &str) -> bool {
    squash(a) == squash(b)
}

fn pull_evidence(
    portflow: &dyn Portflow,
    ledger: &dyn Ledger,
    p: i64,
    me: i64,
    link: &CollectionLink,
    evidence: &crate::portflow_api::Evidence,
    deliver: bool,
    report: &mut Report,
) -> PResult<()> {
    // The outcomes it counts for: goals and sub-goals, by id.
    let mut outcomes: Vec<i64> = match (&evidence.goals, &evidence.sub_goals) {
        (Some(goals), Some(subs)) => goals.iter().chain(subs.iter()).map(|n| n.id).collect(),
        _ => {
            let mut ids: Vec<i64> = portflow.evidence_goals(p, evidence.id)?.into_iter().map(|g| g.id).collect();
            ids.extend(portflow.evidence_sub_goals(p, evidence.id)?.into_iter().map(|s| s.id));
            ids
        }
    };
    outcomes.sort_unstable();
    outcomes.dedup();

    let mut slug = String::new();
    let mut thread: Option<String> = None;
    let mut known: HashMap<i64, Option<i64>> = HashMap::new();
    report.files_touched += ledger
        .write(&mut |conn, _| {
            slug = store::store_evidence(conn, link, evidence, &outcomes)?;
            thread = conn.query_row("SELECT thread_slug FROM evidence WHERE project_slug = ?1 AND slug = ?2", [&link.project, &slug], |r| r.get(0))?;
            known = store::known_versions(conn, &link.project, &slug)?;
            Ok(())
        })
        .map_err(ledger_err)?;

    // Versions Ledger hasn't recorded yet, oldest first. A version that came from Ledger is linked to its deliverables
    // (the same name at major N); one uploaded in Portflow is downloaded below in a study project, and otherwise only
    // named, and opened there.
    let mut versions = portflow.versions(p, evidence.id)?;
    versions.sort_by_key(|v| v.version_number);
    for listed in versions {
        if known.get(&listed.version_number).is_some_and(|pv| *pv == Some(listed.id)) {
            continue;
        }
        let mut version = portflow.version(p, listed.id)?;
        if version.version_type.as_deref().is_some_and(|t| t != "file") {
            version.files.clear();
        }
        let project = link.project.clone();
        let mut in_portflow = 0;
        report.files_touched += ledger
            .write(&mut |conn, _| {
                let mut rows = vec![];
                let mut seen = HashSet::new();
                in_portflow = 0;
                for file in &version.files {
                    let name = if seen.insert(file.name()) { file.name() } else { format!("{} ({})", file.name(), file.id) };
                    let path = store::ledger_file_for(conn, &project, &slug, &file.name(), version.version_number)?;
                    if path.is_none() {
                        in_portflow += 1;
                    }
                    rows.push(VersionFileRow { name, portflow_file: file.id, path });
                }
                store::store_version(conn, &project, &slug, &version, &rows)
            })
            .map_err(ledger_err)?;
        report.new_versions += 1;
        if !deliver {
            report.in_portflow_only += in_portflow;
        }
    }
    // Majors published in Ledger since a version was recorded are linked to it now.
    report.files_touched += ledger.write(&mut |conn, _| store::relink_versions(conn, &link.project, &slug)).map_err(ledger_err)?;
    if deliver {
        deliver_evidence(portflow, ledger, p, &link.project, &slug, report)?;
    }

    // Comments become feedback once the item is in a thread; until then they wait in Portflow.
    let Some(thread) = thread else {
        report.unplaced += 1;
        return Ok(());
    };
    let comments = portflow.evidence_comments(p, evidence.id)?;
    let mut all: Vec<Comment> = vec![];
    for comment in comments {
        let children = if comment.child_count > 0 { portflow.comment_children(p, comment.id)? } else { vec![] };
        all.push(comment);
        all.extend(children);
    }
    let mut new = 0;
    report.files_touched += ledger
        .write(&mut |conn, _| {
            new = 0;
            for comment in &all {
                if store::store_comment(conn, &link.project, &slug, &thread, comment, me)? {
                    new += 1;
                }
            }
            Ok(())
        })
        .map_err(ledger_err)?;
    report.new_feedback += new;
    Ok(())
}
