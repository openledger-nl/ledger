use super::markdown::{escape, string, to_typst};
use super::*;

fn convert(source: &str) -> markdown::Converted {
    to_typst(source, &mut |src| (src == "chart.png").then(|| "/pictures/0.png".to_string()))
}

#[test]
fn a_first_heading_is_the_title_and_the_chapters_move_up() {
    let c = convert("# Project plan\n\nIntro.\n\n## Scope\n\n### Detail\n");
    assert_eq!(c.title.as_deref(), Some("Project plan"));
    assert!(c.body.contains("#heading(level: 1)[Scope]"), "{}", c.body);
    assert!(c.body.contains("#heading(level: 2)[Detail]"), "{}", c.body);
    assert!(!c.body.contains("Project plan"));
}

#[test]
fn chapters_stay_where_they_are_when_there_are_more_first_level_headings() {
    let c = convert("# One\n\n# Two\n\n## Under\n");
    assert_eq!(c.title.as_deref(), Some("One"));
    assert!(c.body.contains("#heading(level: 1)[Two]"));
    assert!(c.body.contains("#heading(level: 2)[Under]"));
}

#[test]
fn text_can_never_be_read_as_typst() {
    let c = convert("Costs $5 #import \"x\" [a] *b* <c> @d // e = f\n");
    assert!(c.body.contains("\\$5 \\#import"), "{}", c.body);
    assert!(c.body.contains("\\[a\\]"), "{}", c.body);
    assert!(c.body.contains("\\/\\/"), "{}", c.body);
    assert_eq!(escape("a-b"), "a\\-b");
    assert_eq!(string("say \"hi\"\\\n"), "\"say \\\"hi\\\"\\\\\\n\"");
}

#[test]
fn notes_links_and_frontmatter_are_left_out_or_reduced_to_their_words() {
    let c = convert("---\ntitle: X\n---\nSee [[Pricing model|the pricing]] and [site](https://example.com). %% for the writer %%\n");
    assert!(c.body.contains("See the pricing and #link(\"https://example.com\")[site]"), "{}", c.body);
    assert!(!c.body.contains("writer"));
    assert!(!c.body.contains("title"));
}

#[test]
fn lists_tables_code_and_pictures_become_calls() {
    let c = convert("- [x] done\n- open\n\n1. one\n\n| A | B |\n|:-|-:|\n| 1 | 2 |\n\n```rust\nfn x() {}\n```\n\n![A chart](chart.png) ![gone](https://x.y/z.png)\n");
    assert!(c.body.contains("#list([#ledger-task(true) done], [open], )"), "{}", c.body);
    assert!(c.body.contains("#enum(start: 1, [one], )"), "{}", c.body);
    assert!(c.body.contains("#table(columns: (auto,) * 1 + (1fr,), align: (left, right,), table.header([A], [B], ), [1], [2], )"), "{}", c.body);
    assert!(c.body.contains("#raw(block: true, lang: \"rust\", \"fn x() {}\")"), "{}", c.body);
    assert!(c.body.contains("#ledger-image(\"/pictures/0.png\", [A chart])"), "{}", c.body);
    assert!(c.body.contains("#ledger-missing-image([gone])"), "{}", c.body);
}

#[test]
fn a_footnote_is_written_where_it_is_referred_to() {
    let c = convert("Claim.[^1]\n\n[^1]: The source.\n");
    assert!(c.body.contains("Claim\\.#footnote[The source\\.]"), "{}", c.body);
}

#[test]
fn picture_addresses_stay_inside_the_project() {
    assert_eq!(resolve_src("deliverables/plans", "chart.png").as_deref(), Some("deliverables/plans/chart.png"));
    assert_eq!(resolve_src("deliverables", "../attachments/a%20b.png").as_deref(), Some("attachments/a b.png"));
    assert_eq!(resolve_src("deliverables", "/attachments/x.png").as_deref(), Some("attachments/x.png"));
    assert_eq!(resolve_src("deliverables", "../../../etc/passwd"), None);
    assert_eq!(resolve_src("deliverables", "https://x.y/z.png"), None);
    assert_eq!(resolve_src("deliverables", "data:image/png;base64,AAAA"), None);
}

#[test]
fn names_and_dates_read_well() {
    assert_eq!(long_date("2026-11-20T10:19:32.123Z"), "20 November 2026");
    assert_eq!(safe_file_name("a/b: c?"), "a-b- c-");
    assert_eq!(file_stem("plans/Project plan.md"), "Project plan");
    assert_eq!(label_name("cyborg"), Some("Cyborg"));
}

/// The whole layout, with every kind of block in it, sets without an error.
#[test]
fn a_document_with_everything_in_it_becomes_a_pdf() {
    let source = "# Plan\n\nIntro with **bold**, *italic*, ~~gone~~, `code` and a [link](https://example.com).[^n]\n\n## One\n\n> A quote.\n\n- a\n  - nested\n- [ ] task\n\n### Two\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```\nplain code\n```\n\n---\n\n## Three\n\n![missing](nope.png)\n\n[^n]: A note.\n";
    let converted = to_typst(source, &mut |_| None);
    let data = Data {
        title: converted.title.clone().unwrap(),
        subtitle: Some("A subtitle".into()),
        version: Some("0.3".into()),
        date: "20 November 2026".into(),
        author: Some("A. Student".into()),
        project: "Police dashboard".into(),
        accent: "#154273".into(),
        lang: "en".into(),
        client: Party { name: Some("Police".into()), logo: None },
        own: Party { name: Some("Fontys ICT".into()), logo: None },
        history: vec![
            HistoryRow { version: "0.1".into(), date: "1 November 2026".into(), by: "Person".into(), note: "-".into(), label: None },
            HistoryRow { version: "0.3".into(), date: "20 November 2026".into(), by: "Claude Code".into(), note: "Tighter".into(), label: Some("Cyborg".into()) },
        ],
        contents: converted.headings >= 3,
    };
    let main = format!("#import \"/ledger.typ\": *\n#show: ledger-doc.with(json(\"/data.json\"))\n\n{}", converted.body);
    let pdf = world::compile(&main, vec![("/data.json".into(), serde_json::to_vec(&data).unwrap())]).unwrap_or_else(|e| panic!("{e}\n{main}"));
    assert!(pdf.starts_with(b"%PDF"));
    if let Ok(dir) = std::env::var("LEDGER_PDF_SAMPLE") {
        std::fs::write(Path::new(&dir).join("sample.pdf"), &pdf).unwrap();
    }
}

/// A published version of a deliverable, with a picture beside it, becomes `exports/<name> v<version>.pdf`; the live
/// file becomes the draft.
#[test]
fn a_published_version_and_the_draft_are_exported_into_the_project() {
    let c = crate::db::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let project = crate::projects::create(&c, "Police dashboard").unwrap().slug;
    let doc = deliverables::create(&c, &project, "Plan", "# Project plan\n\nSee the chart.\n\n![The chart](chart.png)\n").unwrap();
    let pictures = uploads::project_dir(root, &project).join("deliverables");
    fs::create_dir_all(&pictures).unwrap();
    image::RgbImage::from_pixel(40, 20, image::Rgb([21, 66, 115])).save(pictures.join("chart.png")).unwrap();
    let published = deliverables::publish(&c, root, &project, &doc.path, crate::models::Bump::Minor, None).unwrap();
    let version = deliverables::snapshot_path(&published.base_path, published.versions.last().unwrap());

    let prepared = prepare(&c, root, &project, &version).unwrap();
    assert!(prepared.files.iter().any(|(name, _)| name == "/pictures/0.png"), "the picture goes along");
    assert!(prepared.main.contains("#ledger-image(\"/pictures/0.png\", [The chart])"), "{}", prepared.main);
    let data: serde_json::Value = serde_json::from_slice(&prepared.files.iter().find(|(n, _)| n == "/data.json").unwrap().1).unwrap();
    assert_eq!(data["title"], "Project plan");
    assert_eq!(data["version"], "0.1");
    assert_eq!(data["history"].as_array().unwrap().len(), 1);
    let rel = make(prepared).unwrap();
    assert_eq!(rel, "exports/plan v0.1.pdf");
    let pdf = fs::read(uploads::project_dir(root, &project).join(&rel)).unwrap();
    assert!(pdf.starts_with(b"%PDF"));

    assert_eq!(make(prepare(&c, root, &project, &doc.path).unwrap()).unwrap(), "exports/plan (draft).pdf");
}

#[test]
fn only_markdown_can_be_exported() {
    let c = crate::db::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let project = crate::projects::create(&c, "P").unwrap().slug;
    let folder = uploads::project_dir(dir.path(), &project).join("deliverables");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("sheet.xlsx"), b"x").unwrap();
    deliverables::upsert(&c, &project, "sheet.xlsx", 1, "2026-11-20T10:00:00.000Z", None).unwrap();
    assert!(prepare(&c, dir.path(), &project, "sheet.xlsx").is_err());
}

/// The project's branding names the client and you, colours the PDF and puts both logos on the cover.
#[test]
fn the_branding_of_the_project_goes_on_the_cover() {
    let c = crate::db::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let project = crate::projects::create(&c, "Police dashboard").unwrap().slug;
    let doc = deliverables::create(&c, &project, "Plan", "# Project plan\n\n## Why\n\nText.\n\n## How\n\n## When\n").unwrap();
    deliverables::publish(&c, root, &project, &doc.path, crate::models::Bump::Minor, None).unwrap();
    let logo = |name: &str, rgb: [u8; 3]| {
        let file = root.join(name);
        image::RgbImage::from_pixel(300, 100, image::Rgb(rgb)).save(&file).unwrap();
        file
    };
    branding::save(
        &c,
        &project,
        &branding::Branding { author: Some("Sam Student".into()), accent: Some("#154273".into()), own_name: Some("Fontys ICT".into()), client_name: Some("Police".into()), ..Default::default() },
    )
    .unwrap();
    branding::set_logo(&c, root, &project, branding::Side::Own, Some(&logo("fontys.png", [102, 51, 102]))).unwrap();
    branding::set_logo(&c, root, &project, branding::Side::Client, Some(&logo("police.png", [21, 66, 115]))).unwrap();

    let prepared = prepare(&c, root, &project, "plan@v0.1.md").unwrap();
    let data: serde_json::Value = serde_json::from_slice(&prepared.files.iter().find(|(n, _)| n == "/data.json").unwrap().1).unwrap();
    assert_eq!(data["accent"], "#154273");
    assert_eq!(data["author"], "Sam Student");
    assert_eq!(data["client"]["name"], "Police");
    assert_eq!(data["client"]["logo"], "/logos/client.png");
    assert_eq!(data["own"]["logo"], "/logos/own.png");
    assert_eq!(data["history"][0]["by"], "Sam Student");
    assert_eq!(data["contents"], true);
    let rel = make(prepared).unwrap();
    if let Ok(out) = std::env::var("LEDGER_PDF_SAMPLE") {
        fs::copy(uploads::project_dir(root, &project).join(rel), Path::new(&out).join("branded.pdf")).unwrap();
    }
}
