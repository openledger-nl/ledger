//! Typst, built in: what the compiler sees of the world is the layout (Ledger's own, [`LAYOUT`], or one from the vault
//! with the files of its folder), the document Ledger wrote for it, the pictures handed over with it, and the fonts
//! that ship with Ledger (and those in the layout's folder). Nothing is read from disk or the network while it
//! compiles, and no packages can be imported.

use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::Datelike;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::WorldExt;
use typst::foundations::{Bytes, Datetime};
use typst_layout::PagedDocument;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::error::{Error, Result};

/// Ledger's own layout; `/main.typ` imports the layout as `/ledger.typ`.
pub const LAYOUT: &str = include_str!("ledger.typ");

/// The names Ledger's own files go by; a file of the layout's folder with one of them is left out.
const RESERVED: [&str; 6] = ["/main.typ", "/ledger.typ", "/data.json", "/pictures/", "/logos/", "/labels/"];

/// The layout a PDF is set in: its source, what it is called in an error (`pdf/layout.typ`), and the files of its
/// folder (paths relative to it, with `/`): other `.typ` files it imports, pictures, fonts.
pub struct Layout {
    pub source: String,
    pub name: String,
    pub files: Vec<(String, Vec<u8>)>,
}

impl Layout {
    pub fn builtin() -> Layout {
        Layout { source: LAYOUT.to_string(), name: "Ledger's layout".to_string(), files: Vec::new() }
    }
}

fn is_font(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| ["ttf", "otf", "ttc", "otc"].contains(&ext.to_ascii_lowercase().as_str()))
}

/// The fonts the layout names, embedded so a PDF looks the same on every machine.
const FONTS: [&[u8]; 7] = [
    include_bytes!("fonts/Inter-Regular.ttf"),
    include_bytes!("fonts/Inter-Italic.ttf"),
    include_bytes!("fonts/Inter-SemiBold.ttf"),
    include_bytes!("fonts/Inter-Bold.ttf"),
    include_bytes!("fonts/Inter-BoldItalic.ttf"),
    include_bytes!("fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("fonts/JetBrainsMono-Bold.ttf"),
];

struct Shared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

/// The standard library and the fonts, read once.
fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| {
        let fonts: Vec<Font> = FONTS.iter().flat_map(|data| Font::iter(Bytes::new(*data))).collect();
        Shared { library: LazyHash::new(Library::builder().build()), book: LazyHash::new(FontBook::from_fonts(&fonts)), fonts }
    })
}

fn file_id(path: &str) -> Result<FileId> {
    let vpath = VirtualPath::new(path).map_err(|e| Error::export(format!("Not a path the PDF can use: {path} ({e:?})")))?;
    Ok(RootedPath::new(VirtualRoot::Project, vpath).intern())
}

struct Sandbox {
    main: FileId,
    sources: HashMap<FileId, Source>,
    files: HashMap<FileId, Bytes>,
    /// What each source is called in an error.
    names: HashMap<FileId, String>,
    /// Only when the layout brings fonts of its own; else the shared ones.
    book: Option<LazyHash<FontBook>>,
    fonts: Vec<Font>,
}

impl World for Sandbox {
    fn library(&self) -> &LazyHash<Library> {
        &shared().library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.book.as_ref().unwrap_or(&shared().book)
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        match self.sources.get(&id) {
            Some(source) => Ok(source.clone()),
            None if self.files.contains_key(&id) => Err(FileError::NotSource),
            None => Err(FileError::NotFound(id.vpath().get_without_slash().into())),
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(source) = self.sources.get(&id) {
            return Ok(Bytes::from_string(source.text().to_string()));
        }
        self.files.get(&id).cloned().ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        if self.book.is_some() { self.fonts.get(index).cloned() } else { shared().fonts.get(index).cloned() }
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        let today = crate::dates::today();
        Datetime::from_ymd(today.year(), today.month() as u8, today.day() as u8)
    }
}

/// Sets `main` (Typst markup that imports `/ledger.typ`) in `layout`, with `files` beside it (`/data.json`, pictures,
/// logos, each under its absolute name), and gives the PDF.
pub fn compile(main: &str, layout: &Layout, files: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>> {
    let main_id = file_id("/main.typ")?;
    let layout_id = file_id("/ledger.typ")?;
    let mut sources = HashMap::new();
    let mut held = HashMap::new();
    let mut names = HashMap::from([(main_id, "the document".to_string()), (layout_id, layout.name.clone())]);
    let mut fonts = Vec::new();
    // The layout's folder: its other Typst files, its fonts, and anything else it reads, by their names in it.
    let folder = layout.name.rsplit_once('/').map_or("", |(dir, _)| dir);
    for (rel, bytes) in &layout.files {
        let path = format!("/{}", rel.trim_start_matches('/'));
        if RESERVED.iter().any(|r| if r.ends_with('/') { path.starts_with(r) } else { path == *r }) {
            continue;
        }
        if is_font(&path) {
            fonts.extend(Font::iter(Bytes::new(bytes.clone())));
            continue;
        }
        let Ok(id) = file_id(&path) else { continue };
        if path.ends_with(".typ") {
            sources.insert(id, Source::new(id, String::from_utf8_lossy(bytes).into_owned()));
            names.insert(id, if folder.is_empty() { rel.clone() } else { format!("{folder}/{rel}") });
        } else {
            held.insert(id, Bytes::new(bytes.clone()));
        }
    }
    sources.insert(main_id, Source::new(main_id, main.to_string()));
    sources.insert(layout_id, Source::new(layout_id, layout.source.clone()));
    for (path, bytes) in files {
        held.insert(file_id(&path)?, Bytes::new(bytes));
    }
    let (book, fonts) = if fonts.is_empty() {
        (None, Vec::new())
    } else {
        let all: Vec<Font> = shared().fonts.iter().cloned().chain(fonts).collect();
        (Some(LazyHash::new(FontBook::from_fonts(&all))), all)
    };
    let world = Sandbox { main: main_id, sources, files: held, names, book, fonts };
    let document = typst::compile::<PagedDocument>(&world).output.map_err(|errors| failed(&world, errors.as_slice()))?;
    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errors| failed(&world, errors.as_slice()))
}

/// The compiler's complaints, as one message, each with the file and line it is about, so a mistake in a layout of
/// one's own can be found: "pdf/layout.typ, line 12: unknown variable: foo".
fn failed(world: &Sandbox, errors: &[SourceDiagnostic]) -> Error {
    let said: Vec<String> = errors
        .iter()
        .take(3)
        .map(|e| {
            let at = e.span.id().and_then(|id| {
                let name = world.names.get(&id)?;
                let line = world.range(e.span).and_then(|r| world.sources.get(&id)?.lines().byte_to_line(r.start));
                Some(match line {
                    Some(line) => format!("{name}, line {}: ", line + 1),
                    None => format!("{name}: "),
                })
            });
            let hint = e.hints.first().map(|h| format!(" ({})", h.v)).unwrap_or_default();
            format!("{}{}{hint}", at.unwrap_or_default(), e.message)
        })
        .collect();
    Error::export(format!("The PDF could not be made. {}", said.join("; ")))
}
