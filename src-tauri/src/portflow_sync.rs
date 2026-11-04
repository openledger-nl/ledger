//! One Portflow round: read what Portflow has and bring the vault in step with it. Pull only: nothing is sent to
//! Portflow but reads (the access gate in `portflow_api.rs` would refuse anything else anyway).
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
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::portflow_api::{Comment, PResult, Portflow, PortflowError};
use crate::portflow_config::PortflowSettings;
use crate::portflow_store::{self as store, CollectionLink, VersionFileRow};
use crate::state::Core;


/// The vault, as a round sees it.
pub trait Ledger: Send + Sync {
    fn read(&self, f: &mut dyn FnMut(&Connection) -> Result<()>) -> Result<()>;
    /// Changes the database and writes the difference to the vault. Returns how many files that touched.
    fn write(&self, f: &mut dyn FnMut(&Connection, &Path) -> Result<()>) -> Result<usize>;
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
}

/// What a round did.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub outcomes: usize,
    pub people: usize,
    pub collections: usize,
    pub evidence: usize,
    /// Versions recorded this round, and how many of their files are in Portflow only (uploaded there; not copied).
    pub new_versions: usize,
    pub in_portflow_only: usize,
    /// Comments from others that arrived as feedback this round.
    pub new_feedback: usize,
    /// Evidence items that are in no thread yet, so their feedback waits.
    pub unplaced: usize,
    /// Files in the vault this round wrote or removed.
    pub files_touched: usize,
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
        if self.unplaced > 0 {
            parts.push(format!("{} evidence item{} to put in a thread", self.unplaced, if self.unplaced == 1 { "" } else { "s" }));
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
            crate::portflow_gamma::store_sections(conn, &section_list)
        })
        .map_err(ledger_err)?;
    report.people = shares.iter().map(|s| s.invitee.id).collect::<HashSet<_>>().len();

    // The linked collections.
    let sections: HashMap<i64, String> = section_list.into_iter().map(|s| (s.id, s.name)).collect();
    let collections: HashMap<i64, crate::portflow_api::Collection> = portflow.collections(p)?.into_iter().map(|c| (c.id, c)).collect();
    let mut links = vec![];
    ledger
        .read(&mut |conn| {
            links = store::collection_links(conn)?;
            Ok(())
        })
        .map_err(ledger_err)?;

    for link in &links {
        let Some(collection) = collections.get(&link.collection) else {
            report.notes.push(format!("The collection linked to {} for {} isn't in your portfolio any more.", link.project, link.semester));
            continue;
        };
        report.collections += 1;
        let goals = portflow.collection_goals(p, link.collection)?;
        let section = collection.section_id.and_then(|s| sections.get(&s)).cloned().unwrap_or_default();
        report.files_touched += ledger.write(&mut |conn, _| store::store_collection(conn, link, collection, &section, &goals)).map_err(ledger_err)?;

        for item in portflow.collection_evidence(p, link.collection)? {
            report.evidence += 1;
            pull_evidence(portflow, ledger, p, me.id, link, &item.evidence, &mut report)?;
        }
    }
    Ok(report)
}

fn pull_evidence(
    portflow: &dyn Portflow,
    ledger: &dyn Ledger,
    p: i64,
    me: i64,
    link: &CollectionLink,
    evidence: &crate::portflow_api::Evidence,
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

    // Versions Ledger hasn't recorded yet, oldest first. Nothing is downloaded: a version that came from Ledger is linked
    // to its deliverables (the same name at major N), and one uploaded in Portflow is only named, and opened there.
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
        report.in_portflow_only += in_portflow;
    }
    // Majors published in Ledger since a version was recorded are linked to it now.
    report.files_touched += ledger.write(&mut |conn, _| store::relink_versions(conn, &link.project, &slug)).map_err(ledger_err)?;

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
