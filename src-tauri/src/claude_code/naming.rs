//! Naming a prompt from what it asks, the way a chat app names a chat.
//!
//! A prompt has no title to write. Once it is made, Claude Code is asked once, in print mode with no tools and nothing
//! of the project in reach, for a short title for the request, and that becomes the prompt's name. Until it answers, or
//! when it can't (not installed, not signed in, the setting is off), the prompt is called by the start of its text.
//! What is sent is the text of the request and nothing else.

use std::path::Path;
use std::time::Duration;

use super::cli::{self, Search};
use super::config::ClaudeSettings;
use super::runner::{self, Runner, SpawnSpec};
use super::runs;

/// A name is a few words; waiting longer than this for one isn't worth it.
const TIMEOUT: Duration = Duration::from_secs(60);
const MAX_NAME_CHARS: usize = 60;
/// A long request is named from its start.
const MAX_REQUEST_CHARS: usize = 4000;

/// What is sent: the ask, then the request.
pub fn instruction(request: &str) -> String {
    let request: String = request.trim().chars().take(MAX_REQUEST_CHARS).collect();
    format!(
        "Name this request the way a chat app names a chat: reply with only a title of at most six words, in the language of the request, \
         without quotes and without a full stop.\n\nThe request:\n\n{request}\n"
    )
}

/// The name out of what the program printed: its first line, without quotes, labels or a closing full stop. `None` when
/// there is nothing usable.
pub fn clean(output: &str) -> Option<String> {
    let line = output.lines().map(str::trim).find(|l| !l.is_empty())?;
    let line = line.strip_prefix("Title:").or_else(|| line.strip_prefix("title:")).unwrap_or(line);
    // Quotes and a closing full stop come in either order ("Title". and "Title.").
    let mut name = line.to_string();
    loop {
        let next = name
            .trim()
            .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '“' | '”' | '‘' | '’' | '*' | '#'))
            .trim()
            .trim_end_matches(['.', '。'])
            .trim()
            .to_string();
        if next == name {
            break;
        }
        name = next;
    }
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return None;
    }
    let mut out: String = name.chars().take(MAX_NAME_CHARS).collect();
    if name.chars().count() > MAX_NAME_CHARS {
        out = format!("{}…", out.trim_end());
    }
    Some(out)
}

/// Asks Claude Code to name the request. `None` when it can't be reached or says nothing usable: the caller keeps the
/// name the prompt has. Run in `cwd`, which should be a folder with nothing of the vault in it.
pub fn suggest(runner: &dyn Runner, settings: &ClaudeSettings, search: &Search, cwd: &Path, request: &str) -> Option<String> {
    if request.trim().is_empty() {
        return None;
    }
    let program = cli::resolve(&settings.cli_path, search).ok()?;
    let spec = SpawnSpec { env: runs::child_env(&program), program, args: cli::name_args(), cwd: cwd.to_path_buf(), stdin: instruction(request).into_bytes() };
    let out = runner::capture(runner, &spec, TIMEOUT).ok()?;
    if out.timed_out || !out.exit.success() {
        return None;
    }
    clean(&out.stdout)
}
