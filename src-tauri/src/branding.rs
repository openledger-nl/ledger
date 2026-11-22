//! How a project's exported PDFs look and who they name: your name, your own side (your school or employer) with its
//! logo, the client with theirs, and one accent colour. Kept in the frontmatter of the project's `project.md`:
//!
//! ```text
//! branding:
//!   author: Sam Student
//!   accent: '#154273'
//!   own:
//!     name: Fontys ICT
//!     logo: branding/own.png       a file in the project's folder
//!   client:
//!     name: Police                 left out: the project's client, or the first client among its people
//!     logo: branding/client.png
//!   showLabel: true                the version's AI-use label on the cover; left out: only in a study project
//! ```
//!
//! The logos are copied into the project's `branding/` folder when they are chosen, so they sync with the vault and a
//! PDF made on another machine has them too.

use std::fs;
use std::path::{Component, Path};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use crate::db::query_opt;
use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::{projects, uploads};

/// The picture formats a logo can be: what the PDF can show.
pub const LOGO_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "svg", "webp"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Branding {
    pub author: Option<String>,
    /// `#rrggbb`; none for the layout's own.
    pub accent: Option<String>,
    pub own_name: Option<String>,
    /// The logo's path in the project's folder.
    pub own_logo: Option<String>,
    pub client_name: Option<String>,
    pub client_logo: Option<String>,
    /// Whether the cover shows the version's AI-use (MMM) label; `None` for the default: only in a study project.
    pub show_label: Option<bool>,
}

impl Branding {
    fn is_empty(&self) -> bool {
        *self == Branding::default()
    }
}

/// Which logo: yours or the client's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Own,
    Client,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::Own => "own",
            Side::Client => "client",
        }
    }
}

fn trimmed(text: Option<&str>) -> Option<String> {
    text.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

/// `#rrggbb` in lower case, from that or `rrggbb` or `#rgb`; `None` for anything else.
pub fn colour(text: &str) -> Option<String> {
    let hex = text.trim().trim_start_matches('#');
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => Some(format!("#{}", hex.to_ascii_lowercase())),
        3 => Some(format!("#{}", hex.chars().flat_map(|c| [c, c]).collect::<String>().to_ascii_lowercase())),
        _ => None,
    }
}

/// A path inside the project's folder, with `/`; `None` for one that would leave it.
fn inside(path: &str) -> Option<String> {
    let path = path.trim().trim_start_matches('/');
    let p = Path::new(path);
    (!path.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))).then(|| path.replace('\\', "/"))
}

/// Checks the branding and puts it in its canonical form.
fn clean(b: &Branding) -> Result<Branding> {
    let accent = match trimmed(b.accent.as_deref()) {
        Some(text) => Some(colour(&text).ok_or_else(|| Error::invalid("A colour is written like #154273"))?),
        None => None,
    };
    Ok(Branding {
        author: trimmed(b.author.as_deref()),
        accent,
        own_name: trimmed(b.own_name.as_deref()),
        own_logo: b.own_logo.as_deref().and_then(inside),
        client_name: trimmed(b.client_name.as_deref()),
        client_logo: b.client_logo.as_deref().and_then(inside),
        show_label: b.show_label,
    })
}

/// The project's branding; all empty when it has none.
pub fn get(conn: &Connection, project: &str) -> Result<Branding> {
    projects::require(conn, project)?;
    Ok(query_opt(
        conn,
        "SELECT author, accent, own_name, own_logo, client_name, client_logo, show_label FROM project_branding WHERE project_slug = ?1",
        [project],
        |r| {
            Ok(Branding {
                author: r.get(0)?,
                accent: r.get(1)?,
                own_name: r.get(2)?,
                own_logo: r.get(3)?,
                client_name: r.get(4)?,
                client_logo: r.get(5)?,
                show_label: r.get(6)?,
            })
        },
    )?
    .unwrap_or_default())
}

fn store(conn: &Connection, project: &str, b: &Branding) -> Result<()> {
    conn.execute("DELETE FROM project_branding WHERE project_slug = ?1", [project])?;
    if !b.is_empty() {
        conn.execute(
            "INSERT INTO project_branding (project_slug, author, accent, own_name, own_logo, client_name, client_logo, show_label)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![project, b.author, b.accent, b.own_name, b.own_logo, b.client_name, b.client_logo, b.show_label],
        )?;
    }
    Ok(())
}

/// Sets the names and the colour. The logos are set with [`set_logo`]; what `branding` says about them is ignored.
pub fn save(conn: &Connection, project: &str, branding: &Branding) -> Result<Branding> {
    let current = get(conn, project)?;
    let cleaned = clean(branding)?;
    store(conn, project, &Branding { own_logo: current.own_logo, client_logo: current.client_logo, ..cleaned })?;
    get(conn, project)
}

/// Copies the picture at `source` into the project's `branding/` folder as the logo of `side`, or removes that logo
/// when `source` is `None`.
pub fn set_logo(conn: &Connection, root: &Path, project: &str, side: Side, source: Option<&Path>) -> Result<Branding> {
    let mut branding = get(conn, project)?;
    let vault = crate::store::at(root);
    let old = match side {
        Side::Own => branding.own_logo.take(),
        Side::Client => branding.client_logo.take(),
    };
    let new = match source {
        Some(source) => {
            let ext = source.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
            if !LOGO_EXTENSIONS.contains(&ext.as_str()) {
                return Err(Error::invalid("A logo is a PNG, JPEG, SVG or WebP picture"));
            }
            let rel = format!("branding/{}.{ext}", side.word());
            let in_vault = uploads::project_rel(project, &rel);
            // Picking the logo that is already there copies nothing.
            let same = matches!((fs::canonicalize(source), fs::canonicalize(vault.path(&in_vault))), (Ok(a), Ok(b)) if a == b);
            if !same {
                vault.import_file(source, &in_vault).map_err(|e| Error::io(source, e))?;
            }
            Some(rel)
        }
        None => None,
    };
    // The picture it replaces goes, when Ledger put it there itself.
    if let Some(old) = old
        && Some(&old) != new.as_ref()
        && old.starts_with("branding/")
    {
        let _ = vault.remove(&uploads::project_rel(project, &old));
    }
    match side {
        Side::Own => branding.own_logo = new,
        Side::Client => branding.client_logo = new,
    }
    store(conn, project, &branding)?;
    get(conn, project)
}

/// Takes your side from another project: your name, your own name and logo, and the colour. The client stays.
pub fn copy_own(conn: &Connection, root: &Path, project: &str, from: &str) -> Result<Branding> {
    if from == project {
        return Err(Error::invalid("Pick another project to take it from"));
    }
    let source = get(conn, from)?;
    let mut branding = get(conn, project)?;
    branding.author = source.author.clone();
    branding.own_name = source.own_name.clone();
    branding.accent = source.accent.clone();
    store(conn, project, &branding)?;
    let logo = source.own_logo.as_deref().map(|rel| uploads::project_dir(root, from).join(rel)).filter(|p| p.is_file());
    set_logo(conn, root, project, Side::Own, logo.as_deref())
}

// ---- the vault --------------------------------------------------------------------------------------

pub fn export(conn: &Connection, project: &str, meta: &mut Mapping) -> Result<()> {
    let b = get(conn, project)?;
    if b.is_empty() {
        return Ok(());
    }
    let side = |name: &Option<String>, logo: &Option<String>| {
        let mut m = Mapping::new();
        frontmatter::set_opt(&mut m, "name", name.as_deref());
        frontmatter::set_opt(&mut m, "logo", logo.as_deref());
        (!m.is_empty()).then_some(Value::Mapping(m))
    };
    let mut m = Mapping::new();
    frontmatter::set_opt(&mut m, "author", b.author.as_deref());
    frontmatter::set_opt(&mut m, "accent", b.accent.as_deref());
    if let Some(own) = side(&b.own_name, &b.own_logo) {
        frontmatter::set(&mut m, "own", own);
    }
    if let Some(client) = side(&b.client_name, &b.client_logo) {
        frontmatter::set(&mut m, "client", client);
    }
    if let Some(show) = b.show_label {
        frontmatter::set(&mut m, "showLabel", show);
    }
    frontmatter::set(meta, "branding", Value::Mapping(m));
    Ok(())
}

/// Reads the branding leniently: a colour that isn't one is no colour, a logo outside the project is no logo.
pub fn import(conn: &Connection, project: &str, meta: &Mapping) -> Result<()> {
    let Some(m) = meta.get("branding").and_then(Value::as_mapping) else { return store(conn, project, &Branding::default()) };
    let side = |key: &str| m.get(key).and_then(Value::as_mapping);
    let read = Branding {
        author: m.text("author"),
        accent: m.text("accent").and_then(|a| colour(&a)),
        own_name: side("own").and_then(|s| s.text("name")),
        own_logo: side("own").and_then(|s| s.text("logo")),
        client_name: side("client").and_then(|s| s.text("name")),
        client_logo: side("client").and_then(|s| s.text("logo")),
        show_label: m.text("showLabel").and_then(|v| match v.trim().to_lowercase().as_str() {
            "true" | "yes" => Some(true),
            "false" | "no" => Some(false),
            _ => None,
        }),
    };
    store(conn, project, &clean(&read)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_read_in_any_usual_spelling() {
        assert_eq!(colour("#154273").as_deref(), Some("#154273"));
        assert_eq!(colour("ABCDEF").as_deref(), Some("#abcdef"));
        assert_eq!(colour("#f0a").as_deref(), Some("#ff00aa"));
        assert_eq!(colour("blue"), None);
        assert_eq!(colour("#12345"), None);
    }

    #[test]
    fn a_logo_never_points_outside_the_project() {
        assert_eq!(inside("branding/own.png").as_deref(), Some("branding/own.png"));
        assert_eq!(inside("../other/x.png"), None);
        assert_eq!(inside(""), None);
    }

    #[test]
    fn branding_is_kept_in_project_md_and_read_back() {
        let c = crate::db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let p = projects::create(&c, "Police dashboard").unwrap().slug;
        let saved = save(
            &c,
            &p,
            &Branding {
                author: Some(" Sam ".into()),
                accent: Some("154273".into()),
                own_name: Some("Fontys ICT".into()),
                show_label: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(saved.author.as_deref(), Some("Sam"));
        assert_eq!(saved.accent.as_deref(), Some("#154273"));
        assert!(save(&c, &p, &Branding { accent: Some("blue".into()), ..Default::default() }).is_err());

        let logo = dir.path().join("fontys.PNG");
        fs::write(&logo, b"png").unwrap();
        let with_logo = set_logo(&c, dir.path(), &p, Side::Own, Some(&logo)).unwrap();
        assert_eq!(with_logo.own_logo.as_deref(), Some("branding/own.png"));
        assert!(uploads::project_dir(dir.path(), &p).join("branding/own.png").is_file());
        assert!(set_logo(&c, dir.path(), &p, Side::Client, Some(&dir.path().join("x.bmp"))).is_err());

        let mut meta = Mapping::new();
        export(&c, &p, &mut meta).unwrap();
        let text = serde_yaml_ng::to_string(&meta).unwrap();
        assert!(text.contains("branding:") && text.contains("logo: branding/own.png") && text.contains("showLabel: false"), "{text}");
        store(&c, &p, &Branding::default()).unwrap();
        import(&c, &p, &meta).unwrap();
        assert_eq!(get(&c, &p).unwrap(), with_logo);

        // Your side goes along to another project, logo and all; the client stays that project's own.
        let q = projects::create(&c, "Bakery").unwrap().slug;
        let copied = copy_own(&c, dir.path(), &q, &p).unwrap();
        assert_eq!(copied.own_name.as_deref(), Some("Fontys ICT"));
        assert_eq!(copied.own_logo.as_deref(), Some("branding/own.png"));
        assert!(uploads::project_dir(dir.path(), &q).join("branding/own.png").is_file());

        let removed = set_logo(&c, dir.path(), &p, Side::Own, None).unwrap();
        assert_eq!(removed.own_logo, None);
        assert!(!uploads::project_dir(dir.path(), &p).join("branding/own.png").exists());
    }
}
