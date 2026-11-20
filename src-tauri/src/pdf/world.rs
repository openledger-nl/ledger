//! Typst, built in: what the compiler sees of the world is the layout ([`LAYOUT`]), the document Ledger wrote for it,
//! the pictures handed over with it, and the fonts that ship with Ledger. Nothing is read from disk or the network
//! while it compiles, and no packages can be imported.

use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::Datelike;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst_layout::PagedDocument;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::error::{Error, Result};

/// The layout every deliverable is set in; `/main.typ` imports it.
pub const LAYOUT: &str = include_str!("ledger.typ");

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
    main: Source,
    layout: Source,
    files: HashMap<FileId, Bytes>,
}

impl World for Sandbox {
    fn library(&self) -> &LazyHash<Library> {
        &shared().library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &shared().book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else if id == self.layout.id() {
            Ok(self.layout.clone())
        } else {
            Err(FileError::NotSource)
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() || id == self.layout.id() {
            return Ok(Bytes::from_string(self.source(id)?.text().to_string()));
        }
        self.files.get(&id).cloned().ok_or_else(|| FileError::NotFound(format!("{id:?}").into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        shared().fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        let today = chrono::Local::now().date_naive();
        Datetime::from_ymd(today.year(), today.month() as u8, today.day() as u8)
    }
}

/// Sets `main` (Typst markup that imports `/ledger.typ`) with `files` beside it (`/data.json`, pictures, logos, each
/// under its absolute name), and gives the PDF.
pub fn compile(main: &str, files: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>> {
    let mut held = HashMap::new();
    for (path, bytes) in files {
        held.insert(file_id(&path)?, Bytes::new(bytes));
    }
    let world = Sandbox {
        main: Source::new(file_id("/main.typ")?, main.to_string()),
        layout: Source::new(file_id("/ledger.typ")?, LAYOUT.to_string()),
        files: held,
    };
    let document = typst::compile::<PagedDocument>(&world).output.map_err(|errors| failed(errors.as_slice()))?;
    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errors| failed(errors.as_slice()))
}

/// The compiler's complaints, as one message.
fn failed(errors: &[SourceDiagnostic]) -> Error {
    let said: Vec<String> = errors.iter().take(3).map(|e| e.message.to_string()).collect();
    Error::export(format!("The PDF could not be made: {}", said.join("; ")))
}
