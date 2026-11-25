// Report layout shared by every report. Data arrives as JSON in sys.inputs.data, with every cell
// already formatted as text: `reports` holds one report, or every report for "Export all" (each
// starts on a new page). Rendered by services/api/src/services/report_export.rs.
#let data = json(bytes(sys.inputs.data))
#let muted = rgb("#6b7280")
#let rule = rgb("#e5e7eb")

#set document(title: data.title, author: data.business_name)
#set text(font: "Inter", size: 8.5pt, fill: rgb("#111827"), lang: "en", region: "nz")
#set par(leading: 0.55em)
#set page(
  paper: "a4",
  margin: (x: 15mm, top: 15mm, bottom: 18mm),
  footer: context [
    #set text(7pt, fill: muted)
    #grid(
      columns: (1fr, auto),
      [#data.footer],
      [Page #counter(page).display() of #counter(page).final().first()],
    )
  ],
)

#let render(data) = {
let accent = rgb(data.accent)
[
// ─── Header ─────────────────────────────────────────────────────────────────
#grid(
  columns: (1fr, auto),
  gutter: 12pt,
  [
    #text(8pt, weight: "semibold", fill: muted, upper(data.business_name))
    #v(2pt)
    #text(15pt, weight: "bold", fill: accent, data.title)
    #for line in data.subtitle [
      #v(1pt)
      #text(9pt, line)
    ]
  ],
  align(right)[
    #set text(8pt, fill: muted)
    #for line in data.meta [
      #line \
    ]
  ],
)
#v(4pt)
#line(length: 100%, stroke: 0.6pt + accent)
#v(6pt)

// ─── Sections ───────────────────────────────────────────────────────────────
#for section in data.sections [
  #block(breakable: true, below: 12pt)[
    #text(10pt, weight: "semibold", section.title)
    #if section.description != "" [
      #v(1pt)
      #text(7.5pt, fill: muted, section.description)
    ]
    #v(3pt)
    #if section.rows.len() == 0 and section.totals == none [
      #text(8pt, fill: muted, "Nothing in this period.")
    ] else [
      #let aligns = section.columns.map(c => if c.right { right } else { left })
      #table(
        columns: section.columns.map(c => if c.right { auto } else { 1fr }),
        stroke: (x, y) => if y == 0 { (bottom: 0.6pt + rule) } else { (bottom: 0.3pt + rule) },
        inset: (x: 4pt, y: 3.5pt),
        align: (x, y) => aligns.at(x) + horizon,
        table.header(..section.columns.map(c => text(7.5pt, weight: "semibold", fill: muted, c.label))),
        ..section.rows.flatten(),
        ..if section.totals != none { section.totals.map(t => text(weight: "semibold", t)) } else { () },
      )
    ]
  ]
]

#if data.notes.len() > 0 [
  #v(4pt)
  #set text(7.5pt, fill: muted)
  #for note in data.notes [
    #note \
  ]
]

]
}

#for report in data.reports {
  page(flipped: report.landscape, render(report))
}
