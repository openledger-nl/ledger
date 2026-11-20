// The layout of a deliverable exported as a PDF. This is a Typst file (https://typst.app/docs): change anything in it
// and export again to see the result.
//
// To make your own: in Ledger, open a project's Templates tab, go to "On exported PDFs" and click "Customise". That copies
// this file to `pdf/layout.typ` in the vault (every project) or `projects/<project>/pdf/layout.typ` (one project); a
// project's own layout comes first, then the vault's, then Ledger's. "Back to the standard layout" deletes your copy.
// Other files in that `pdf/` folder can be used by their name: pictures (`image("background.png")`), other Typst files
// (`#import "parts.typ": *`) and fonts (a `.ttf` or `.otf` there can be named in `text(font: ...)`). Packages from
// Typst Universe can't be imported, and nothing outside the folder can be read.
//
// What Ledger hands over. It writes a document that imports this file and starts with
//   #show: ledger-doc.with(json("/data.json"))
// so `ledger-doc(data, doc)` below must stay, with `data` holding (each field that may be missing is `none`):
//   title, subtitle         the cover's title, and a line under it
//   version                 "0.3", or none for a draft (the live file)
//   date                    "20 November 2026": the version's day, or today for a draft
//   author                  your name; project: the project's name
//   accent                  the project's colour, "#e07a3f" when it has none of its own
//   lang                    "en", or the deliverable's frontmatter `lang`
//   client, own             (name: ..., logo: ...): who it is for and who made it; a logo is a path to read with image()
//   history                 the published versions, oldest first: (version, date, by, note, label)
//   contents                true when the document has three headings or more
//   label                   none, or (name: "Cyborg", picture: path, intended: bool): the AI-use label for the cover
// `doc` is the document itself. It calls the helpers below (ledger-image, ledger-missing-image, ledger-task,
// ledger-rule), so they must stay too, and everything else is ordinary Typst: headings, lists, tables, quotes, raw.

// ---- things to tweak first --------------------------------------------------------------------------------

#let cover-logo-height = 1.1cm
#let footer-logo-height = 0.45cm
#let ink = rgb("#2b323b")
#let ink-2 = rgb("#48535f")
#let muted = rgb("#6b7785")
#let line-colour = rgb("#d5dce3")
#let paper-2 = rgb("#f3f0ec")

// ---- helpers the converted document calls ------------------------------------------------------------

/// A picture: as large as it is, but never wider than the column; its alt text is its caption.
#let ledger-image(path, alt) = figure(
  layout(size => {
    let img = image(path)
    if measure(img).width > size.width { image(path, width: 100%) } else { img }
  }),
  caption: alt,
)

/// A picture that could not be read (a web address, a file that isn't there): its alt text, in a frame.
#let ledger-missing-image(alt) = block(
  width: 100%,
  inset: 10pt,
  stroke: (paint: line-colour, dash: "dashed"),
  radius: 4pt,
  text(fill: muted, size: 9pt)[Picture: #alt],
)

/// A task list's box.
#let ledger-task(done) = [#box(
  width: 0.8em,
  height: 0.8em,
  baseline: 0.1em,
  stroke: 0.7pt + muted,
  radius: 2pt,
  if done { align(center + horizon, text(size: 0.7em, weight: "bold", fill: ink)[x]) },
)#h(0.45em)]

/// A horizontal rule (`---`).
#let ledger-rule() = block(above: 12pt, below: 12pt, line(length: 100%, stroke: 0.5pt + line-colour))

// ---- the document -----------------------------------------------------------------------------------------

#let ledger-doc(data, doc) = {
  let accent = rgb(data.accent)
  let accent-dark = accent.darken(18%)
  let version-text = if data.version == none { "Draft" } else { "Version " + data.version }

  set document(title: data.title, author: if data.author == none { () } else { data.author })
  set text(font: "Inter", size: 10.5pt, fill: ink, lang: data.lang)
  set par(leading: 0.75em, spacing: 1.1em, justify: false)

  show heading: set text(fill: ink, weight: "bold")
  show heading.where(level: 1): it => block(
    width: 100%,
    above: 26pt,
    below: 13pt,
    inset: (bottom: 5pt),
    stroke: (bottom: 0.74pt + accent),
    text(size: 20pt, it.body),
  )
  show heading.where(level: 2): it => block(above: 17pt, below: 6pt, text(size: 14pt, fill: ink-2, it.body))
  show heading.where(level: 3): it => block(above: 12pt, below: 5pt, text(size: 11.5pt, it.body))
  show heading.where(level: 4): it => block(above: 10pt, below: 4pt, text(size: 10.5pt, it.body))

  show strong: set text(fill: ink)
  show link: set text(fill: accent-dark)
  show quote.where(block: true): it => block(
    above: 9pt,
    below: 14pt,
    inset: (left: 11pt, y: 2pt),
    stroke: (left: 3pt + accent),
    text(style: "italic", fill: rgb("#3e4c59"), it.body),
  )
  show raw: set text(font: "JetBrains Mono", size: 0.9em)
  show raw.where(block: true): it => block(width: 100%, inset: 9pt, radius: 4pt, fill: paper-2, text(size: 8.5pt, it))
  show raw.where(block: false): it => box(fill: paper-2, inset: (x: 3pt), outset: (y: 2pt), radius: 2pt, it)
  set list(indent: 4pt, body-indent: 7pt, marker: ([•], [◦], [‣]))
  set enum(indent: 4pt, body-indent: 7pt)
  set table(
    stroke: (x, y) => (bottom: 0.5pt + line-colour),
    fill: (x, y) => if y == 0 { paper-2 },
    inset: (x: 6pt, y: 5pt),
  )
  show table: set text(size: 10pt)
  show table.cell.where(y: 0): set text(weight: "bold", fill: ink)
  show figure.caption: set text(size: 9pt, style: "italic", fill: muted)
  show footnote.entry: set text(size: 8.5pt)

  // ---- the cover ----
  page(margin: (x: 2.5cm, top: 2cm, bottom: 2cm), header: none, footer: none, {
    // The logos: the client's on the left, your own on the right.
    let logo(path) = if path != none { image(path, height: cover-logo-height, fit: "contain") }
    grid(
      columns: (1fr, 1fr),
      align(left + horizon, logo(data.client.logo)), align(right + horizon, logo(data.own.logo)),
    )
    v(1fr)
    box(width: 3cm, height: 0.22cm, fill: accent)
    v(14pt)
    block(below: 14pt, par(leading: 0.3em, text(size: 34pt, weight: "bold", data.title)))
    if data.subtitle != none {
      block(below: 0pt, par(leading: 0.5em, text(size: 15pt, fill: muted, data.subtitle)))
    }
    v(2.2cm)
    set text(size: 10.5pt, fill: rgb("#5c6670"))
    set par(spacing: 0.55em)
    let meta-line(label, value) = if value != none and value != "" { par[#text(fill: muted, label) #h(4pt) #value] }
    meta-line("For", data.client.name)
    meta-line("By", (data.author, data.own.name).filter(x => x != none).join(", "))
    meta-line("Project", data.project)
    par(version-text)
    par(data.date)
    v(1.6fr)
    // The AI-use label, bottom left: the official Me & My Machine picture with its name.
    if data.label != none {
      place(bottom + left, grid(
        columns: (auto, auto),
        column-gutter: 10pt,
        align: horizon,
        image(data.label.picture, height: 1.5cm),
        {
          set par(spacing: 0.4em)
          text(size: 8.5pt, fill: muted, if data.label.intended [Intended AI use] else [AI use])
          linebreak()
          text(size: 11pt, weight: "bold", fill: ink, data.label.name)
          linebreak()
          text(size: 7.5pt, fill: muted)[Me & My Machine label - mmmlabel.tech]
        },
      ))
    }
  })

  // ---- every page after it: a header and a footer ----
  set page(
    paper: "a4",
    margin: (x: 2.5cm, top: 2.5cm, bottom: 2.2cm),
    header: {
      set text(size: 8.5pt, fill: muted)
      block(width: 100%, inset: (bottom: 4pt), stroke: (bottom: 0.5pt + line-colour), grid(
        columns: (1fr, auto),
        data.title, if data.author != none { data.author },
      ))
    },
    footer: context {
      set text(size: 8.5pt, fill: muted)
      // Both logos, small, on the left; then the subtitle; the version and page on the right.
      let logos = (data.client.logo, data.own.logo).filter(l => l != none).map(l => image(l, height: footer-logo-height, fit: "contain"))
      grid(
        columns: (auto, 1fr, auto),
        column-gutter: if logos.len() > 0 { 10pt } else { 0pt },
        align: horizon,
        stack(dir: ltr, spacing: 6pt, ..logos),
        if data.subtitle != none { data.subtitle },
        [#version-text | Page #counter(page).display() of #counter(page).final().first()],
      )
    },
  )
  counter(page).update(2)

  // ---- the version history and the contents ----
  let front = data.history.len() > 0 or data.contents
  if data.history.len() > 0 {
    heading(level: 1, outlined: false)[Version history]
    let labelled = data.history.any(v => v.label != none)
    let head = ([Version], [Date], [Made by], [Changes]) + if labelled { ([AI use],) } else { () }
    table(
      columns: if labelled { (auto, auto, auto, 1fr, auto) } else { (auto, auto, auto, 1fr) },
      table.header(..head),
      ..data
        .history
        .map(v => (v.version, v.date, v.by, v.note) + if labelled { (if v.label == none { [-] } else { v.label },) } else { () })
        .flatten()
    )
    v(18pt)
  }
  if data.contents {
    show outline.entry.where(level: 1): it => block(above: 9pt, strong(it))
    outline(title: [Contents], depth: 3, indent: 1.2em)
  }
  if front { pagebreak() }

  doc
}
