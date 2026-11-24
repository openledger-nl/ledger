mod activity;
mod applog;
mod board;
mod branding;
mod claude_code;
mod clients;
mod commands;
mod conflicts;
mod crypto;
mod dates;
mod db;
mod daily;
mod deliverable_facts;
mod deliverables;
mod devupdates;
mod error;
mod feedback;
mod frontmatter;
mod history;
mod ids;
mod images;
mod marker;
mod links;
mod markdown;
mod models;
mod onboarding;
mod people;
mod pdf;
mod posts;
mod project_facts;
mod projects;
mod prompts;
mod provenance;
mod screenshot;
mod search;
mod semester_templates;
mod semesters;
mod settings;
mod state;
mod stories;
mod store;
mod templates;
mod threads;
mod updates;
mod uploads;
mod vault;
mod vault_crypto;
mod watcher;

// ---- Trello sync (feat/trello-sync) ----
mod trello;
mod trello_api;
mod trello_card;
mod trello_config;
mod trello_ledger;
mod trello_sync;
// ---- end Trello sync ----

// ---- Portflow (feat/portflow-integration) ----
mod portflow;
mod portflow_api;
mod portflow_auth;
mod portflow_epsilon;
mod portflow_evidence_text;
mod portflow_config;
mod portflow_html;
mod portflow_login;
mod portflow_store;
mod portflow_sync;
mod portflow_vault;
// ---- end Portflow ----

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_applog;
#[cfg(test)]
mod tests_claude_code;
#[cfg(test)]
mod tests_cloud;
#[cfg(test)]
mod tests_encrypted;
#[cfg(test)]
mod tests_trello;
#[cfg(test)]
mod tests_crypto;
#[cfg(test)]
mod tests_devupdates;
#[cfg(test)]
mod tests_onboarding;
#[cfg(test)]
mod tests_portflow;
#[cfg(test)]
mod tests_v1;
#[cfg(test)]
mod tests_provenance;
#[cfg(test)]
mod tests_history;
#[cfg(test)]
mod tests_screenshot;
#[cfg(test)]
mod tests_updates;
#[cfg(test)]
mod tests_vault;
#[cfg(test)]
mod tests_vaults;


use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Keeps the main window on Ledger itself: a navigation away from the app (a file dropped where nothing caught it, a link
/// that slipped through) is refused, so the window can never end up showing a PDF instead of Ledger. Links outside are
/// opened with the system's browser (the opener plugin), never in this window. Other windows (the Portflow sign-in, the
/// screenshot overlay) are left alone.
fn stay_in_the_app<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("stay-in-the-app")
        .on_navigation(|webview, url| webview.label() != "main" || is_app_url(url))
        .build()
}

/// The app's own pages: the bundled ones (tauri://, or http(s)://tauri.localhost on Windows) and, in development, the dev
/// server on localhost.
fn is_app_url(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" => true,
        "http" | "https" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "tauri.localhost")),
        _ => false,
    }
}

pub fn run() {
    let builder = tauri::Builder::default();
    // First, so a second launch hands its arguments over before anything else starts. That is how a picture
    // opened with Ledger (Flameshot's "Open with", a file manager) reaches the window that is already open.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
        screenshot::reveal_main(app);
        screenshot::open_files(app, args.get(1..).unwrap_or_default(), std::path::Path::new(&cwd));
    }));
    // Looking for and installing new releases (`updates.rs`): the plugin is in release builds only. A development build
    // follows its git branch instead (`devupdates.rs`), without it. `updates::BUILT_IN` says the same to the rest of the app.
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    #[expect(clippy::expect_used, reason = "without its window there is no Ledger to fall back to: stopping is all it can do")]
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(stay_in_the_app())
        .setup(|app| {
            // The vault is loaded into memory before the window shows anything.
            // LEDGER_CONFIG_DIR moves the per-machine settings (vault location, screenshot hotkey, update checks) for testing.
            let config_dir = match std::env::var_os("LEDGER_CONFIG_DIR") {
                Some(dir) => dir.into(),
                None => app.path().app_config_dir()?,
            };
            // The log first, so whatever goes wrong from here on is in it. LEDGER_CONFIG_DIR moves it along.
            let log_dir = match std::env::var_os("LEDGER_CONFIG_DIR") {
                Some(_) => config_dir.join("logs"),
                None => app.path().app_log_dir()?,
            };
            let log_handle = app.handle().clone();
            applog::init(
                &log_dir,
                settings::load_config(&config_dir).log_max_bytes.unwrap_or(applog::DEFAULT_MAX_BYTES),
                Box::new(move |entry| {
                    let _ = log_handle.emit("log-entry", entry);
                }),
            );
            applog::info("app", format!("Ledger {} started on {} {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH));
            let core = state::Core::open(config_dir.clone())?;
            let watched = core.open.then(|| core.vault.root().to_path_buf());
            let core = Arc::new(Mutex::new(core));
            let trello_core = core.clone(); // for the Trello sync (feat/trello-sync), started below
            let portflow_core = core.clone(); // for Portflow (feat/portflow-integration), started below

            app.manage(state::App::new(core, screenshot::Screenshots::new()));
            // What the cloud storage brings in is read in once the folder is quiet again.
            let watch_handle = app.handle().clone();
            let watcher = watcher::VaultWatcher::start(Arc::new(move || commands::vault_changed_outside(&watch_handle)));
            if let Some(root) = watched {
                watcher.watch(&root);
            }
            app.manage(watcher);
            trello::start_app(app.handle(), trello_core); // feat/trello-sync: its own worker, needs the App state above
            portflow::start_app(app.handle(), portflow_core); // feat/portflow-integration: runs only while switched on
            // Looks for new releases now and then (release builds only); it asks the syncs above whether they are busy.
            updates::start(app.handle(), config_dir);
            screenshot::register_saved(app.handle());
            // Launched with a picture to attach: it waits until the window asks for it.
            let args: Vec<String> = std::env::args().skip(1).collect();
            screenshot::open_files(app.handle(), &args, &std::env::current_dir().unwrap_or_default());
            Ok(())
        })
        .on_window_event(claude_code::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::render_markdown,
            commands::list_links,
            commands::search,
            commands::list_activity,
            commands::list_semesters,
            commands::create_semester,
            commands::update_semester,
            commands::delete_semester,
            commands::list_projects,
            commands::create_project,
            commands::rename_project,
            commands::get_project,
            commands::save_readme,
            commands::get_project_meta,
            commands::list_project_meta,
            commands::save_project_meta,
            commands::list_project_people,
            commands::list_people,
            commands::list_semester_templates,
            commands::save_semester_template,
            commands::delete_semester_template,
            commands::list_clients,
            commands::create_client,
            commands::update_client,
            commands::delete_client,
            commands::get_person,
            commands::create_person,
            commands::update_person,
            commands::delete_person,
            commands::set_post_meta,
            commands::get_deliverable_meta,
            commands::list_deliverable_meta,
            commands::save_deliverable_meta,
            commands::get_deliverable_description,
            commands::save_deliverable_description,
            commands::next_deliverable_number,
            commands::confirm_label,
            commands::save_project_people,
            commands::get_project_branding,
            commands::save_project_branding,
            commands::set_project_logo,
            commands::copy_project_branding,
            commands::delete_project,
            commands::create_thread,
            commands::rename_thread,
            commands::get_thread,
            commands::delete_thread,
            commands::link_threads,
            commands::unlink_threads,
            commands::add_post,
            commands::update_post,
            commands::delete_post,
            commands::create_prompt,
            commands::update_prompt,
            commands::delete_prompt,
            commands::link_prompt_deliverable,
            commands::unlink_prompt_deliverable,
            commands::create_prompt_deliverable,
            commands::list_deliverable_prompts,
            commands::suggest_prompt_base,
            commands::attach_prompt_feedback,
            commands::detach_prompt_feedback,
            commands::add_feedback,
            commands::list_all_feedback,
            commands::list_daily_notes,
            commands::get_daily_note,
            commands::save_daily_note,
            commands::delete_daily_note,
            commands::record_feedback,
            commands::set_feedback_status,
            commands::update_feedback,
            commands::delete_feedback,
            commands::upload_files,
            commands::upload_dropped,
            commands::save_pasted_image,
            commands::read_project_image,
            commands::preview_dropped_image,
            commands::list_project_uploads,
            commands::update_upload,
            commands::delete_upload,
            commands::read_project_file,
            commands::open_project_file,
            commands::open_attachments_folder,
            commands::list_deliverables,
            commands::get_deliverable,
            commands::create_deliverable,
            commands::save_deliverable,
            commands::deliverable_changes,
            commands::deliverable_authors,
            commands::deliverable_live_changes,
            commands::publish_deliverable,
            commands::export_deliverable_pdf,
            commands::pdf_layout,
            commands::customise_pdf_layout,
            commands::reset_pdf_layout,
            commands::open_pdf_layout,
            commands::get_provenance,
            commands::update_version_record,
            commands::delete_deliverable,
            commands::rename_deliverable,
            commands::continue_deliverable_in_markdown,
            commands::separate_deliverable,
            commands::list_templates,
            commands::project_templates,
            commands::effective_templates,
            commands::create_template,
            commands::update_template,
            commands::override_template,
            commands::delete_template,
            commands::template_reader_presets,
            commands::template_skeleton,
            commands::list_stories,
            commands::create_story,
            commands::update_story,
            commands::delete_story,
            commands::link_story_thread,
            commands::unlink_story_thread,
            commands::get_board,
            commands::create_task,
            commands::update_task,
            commands::delete_task,
            commands::add_column,
            commands::rename_column,
            commands::delete_column,
            commands::get_settings,
            applog::about_info,
            applog::log_write,
            applog::log_read,
            applog::log_clear,
            applog::log_set_max_bytes,
            applog::capture_status,
            applog::capture_start,
            applog::capture_debug,
            applog::capture_stop,
            applog::capture_save,
            // ---- Trello sync (feat/trello-sync) ----
            trello::trello_get,
            trello::trello_status,
            trello::trello_test,
            trello::trello_save_credentials,
            trello::trello_disconnect,
            trello::trello_boards,
            trello::trello_board_members,
            trello::trello_save_member,
            trello::trello_link_project,
            trello::trello_project_mapping,
            trello::trello_set_column_list,
            trello::trello_sync_now,
            trello::trello_relink_item,
            // ---- end Trello sync ----
            // ---- Portflow (feat/portflow-integration) ----
            portflow::portflow_get,
            portflow::portflow_status,
            portflow::portflow_set_enabled,
            portflow::portflow_set_launch_url,
            portflow::portflow_set_access,
            portflow::portflow_sign_in,
            portflow::portflow_sign_out,
            portflow::portflow_save_login,
            portflow::portflow_forget_login,
            portflow::portflow_sync_now,
            portflow::portflow_collections,
            portflow::portflow_link_collection,
            portflow::portflow_import_collection,
            portflow::portflow_open,
            portflow::portflow_people,
            portflow::portflow_project,
            portflow::portflow_thread_evidence,
            portflow::portflow_place_evidence,
            portflow::portflow_save_evidence_description,
            portflow::portflow_recent_feedback,
            portflow::portflow_epsilon,
            portflow::portflow_sections,
            portflow::portflow_link_section,
            portflow::portflow_set_intended,
            portflow::portflow_evidence_choices,
            // ---- end Portflow ----
            commands::set_vault_path,
            commands::list_vaults,
            commands::create_vault,
            commands::open_vault,
            commands::forget_vault,
            commands::reload_vault,
            commands::set_feedback_from,
            commands::check_vault_changes,
            commands::list_conflicts,
            commands::unlock_vault,
            commands::vault_encryption,
            commands::change_passphrase,
            commands::recovery_key,
            commands::forget_vault_key,
            commands::encrypted_copy,
            commands::resolve_conflict,
            commands::save_screenshot_hotkey,
            commands::save_screenshot_tool,
            commands::layout_settings,
            commands::save_sidebar_fold,
            commands::pending_screenshot,
            commands::take_screenshot,
            commands::pause_screenshot_hotkey,
            commands::screenshot_image,
            commands::screenshot_overlay_ready,
            commands::screenshot_finish,
            commands::screenshot_cancel,
            commands::pending_screenshot_image,
            commands::attach_screenshot,
            commands::discard_screenshot,
            claude_code::claude_code_settings,
            claude_code::save_claude_code_settings,
            claude_code::check_claude_code,
            claude_code::claude_code_preview,
            claude_code::claude_code_start,
            claude_code::claude_code_cancel,
            claude_code::claude_code_runs,
            claude_code::claude_code_run,
            onboarding::onboarding_status,
            onboarding::skip_onboarding_step,
            onboarding::set_onboarding_purpose,
            onboarding::finish_onboarding,
            onboarding::restart_onboarding,
            updates::update_status,
            updates::save_update_settings,
            updates::check_for_updates,
            updates::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {
            // macOS doesn't pass opened files as arguments: they arrive here, at launch and while running.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &_event {
                let paths: Vec<String> = urls.iter().filter_map(|u| u.to_file_path().ok()).map(|p| p.display().to_string()).collect();
                screenshot::open_files(_app, &paths, std::path::Path::new("/"));
            }
        });
}

#[cfg(test)]
mod tests_navigation {
    use super::is_app_url;

    #[test]
    fn the_main_window_stays_on_the_app() {
        for ok in ["tauri://localhost/projects/x", "http://localhost:1420/", "http://tauri.localhost/", "https://tauri.localhost/a"] {
            assert!(is_app_url(&ok.parse().unwrap()), "{ok}");
        }
        for no in ["file:///home/me/plan.pdf", "https://fontysict.instructure.com/", "http://example.com/", "data:text/html,hi"] {
            assert!(!is_app_url(&no.parse().unwrap()), "{no}");
        }
    }
}
