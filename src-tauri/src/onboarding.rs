//! The guide a new client gets on the home page: bring in a vault that exists, or set one up step by step (a
//! semester, sync to GitHub, a first thread, a first deliverable).
//!
//! Whether the guide shows is a fact about this machine's copy of Ledger, not about a vault, so it is kept in
//! `config.json` with the vault's location (see `settings`). What each step needs is read from the vault by the page,
//! so the steps cannot disagree with what is really there, and a vault brought in from elsewhere finds them done
//! already. All that is kept here is what the vault cannot say: that the guide is on or finished, and which of the
//! steps that can wait were put off.

use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::query_opt;
use crate::error::{Error, Result};
use crate::settings;
use crate::state::App;

/// The steps that can be left for later: the vault works without them.
pub const SKIPPABLE: [&str; 2] = ["semester", "sync"];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// Not looked at yet: a machine that has never started Ledger, or one that did before the guide existed.
    #[default]
    Undecided,
    Active,
    Done,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OnboardingConfig {
    pub stage: Stage,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingView {
    /// The guide is on the home page.
    pub active: bool,
    /// The steps that were put off (see [`SKIPPABLE`]).
    pub skipped: Vec<String>,
}

fn view(config: &OnboardingConfig) -> OnboardingView {
    OnboardingView { active: config.stage == Stage::Active, skipped: config.skipped.clone() }
}

/// Whether the vault holds anything of its own yet: a project or a semester. A vault that does has been set up already,
/// by hand, by an older version, or on another machine.
pub fn has_content(conn: &Connection) -> Result<bool> {
    let found = query_opt(conn, "SELECT 1 WHERE EXISTS (SELECT 1 FROM projects) OR EXISTS (SELECT 1 FROM semesters)", [], |r| r.get::<_, i64>(0))?;
    Ok(found.is_some())
}

/// Where the guide stands. The first time it is asked, the answer is decided and kept: a vault with something in it is
/// not a new client's, and one without is. After that it only changes when the person finishes the guide or asks for it
/// again, so setting up the vault never makes the guide vanish halfway.
pub fn status(config_dir: &Path, vault_has_content: bool) -> Result<OnboardingView> {
    let mut config = settings::load_config(config_dir).onboarding;
    if config.stage == Stage::Undecided {
        config.stage = if vault_has_content { Stage::Done } else { Stage::Active };
        settings::update_config(config_dir, |c| c.onboarding.stage = config.stage)?;
    }
    Ok(view(&config))
}

/// Puts a step off. Only the steps that can wait can be.
pub fn skip(config_dir: &Path, step: &str) -> Result<OnboardingView> {
    if !SKIPPABLE.contains(&step) {
        return Err(Error::invalid(format!("The step “{step}” can't be skipped")));
    }
    settings::update_config(config_dir, |c| {
        if !c.onboarding.skipped.iter().any(|s| s == step) {
            c.onboarding.skipped.push(step.to_string());
        }
    })?;
    Ok(view(&settings::load_config(config_dir).onboarding))
}

/// The guide is over: finished, or set aside. It stays over until it is asked for again.
pub fn finish(config_dir: &Path) -> Result<OnboardingView> {
    settings::update_config(config_dir, |c| c.onboarding.stage = Stage::Done)?;
    Ok(view(&settings::load_config(config_dir).onboarding))
}

/// Shows the guide again, from the start.
pub fn restart(config_dir: &Path) -> Result<OnboardingView> {
    settings::update_config(config_dir, |c| c.onboarding = OnboardingConfig { stage: Stage::Active, skipped: vec![] })?;
    Ok(view(&settings::load_config(config_dir).onboarding))
}

// ---- commands ---------------------------------------------------------------------------------------

#[tauri::command]
pub fn onboarding_status(app: State<App>) -> Result<OnboardingView> {
    let content = app.read(|conn, _| has_content(conn))?;
    let dir = app.lock().config_dir.clone();
    status(&dir, content)
}

#[tauri::command]
pub fn skip_onboarding_step(app: State<App>, step: String) -> Result<OnboardingView> {
    let dir = app.lock().config_dir.clone();
    skip(&dir, &step)
}

#[tauri::command]
pub fn finish_onboarding(app: State<App>) -> Result<OnboardingView> {
    let dir = app.lock().config_dir.clone();
    finish(&dir)
}

#[tauri::command]
pub fn restart_onboarding(app: State<App>) -> Result<OnboardingView> {
    let dir = app.lock().config_dir.clone();
    restart(&dir)
}
