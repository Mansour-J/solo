// Invoice and quote layout. Data arrives as JSON in sys.inputs.data (pre-formatted strings), and an
// optional logo as bytes in sys.inputs.logo. Rendered by services/api/src/services/pdf.rs.
#let data = json(bytes(sys.inputs.data))
#let logo = sys.inputs.at("logo", default: none)
#let accent = rgb(data.accent)
#let muted = rgb("#6b7280")
#let rule = rgb("#e5e7eb")

#set document(title: data.title + " " + data.number, author: data.business.name)
#set text(font: "Inter", size: 9.5pt, fill: rgb("#111827"), lang: "en", region: "nz")
#set par(leading: 0.6em)
#set page(
  paper: "a4",
  margin: (x: 18mm, top: 16mm, bottom: 20mm),
  background: if data.watermark != "" {
    place(center + horizon, rotate(-28deg, text(72pt, weight: "bold", fill: rgb(17, 24, 39, 14), data.watermark)))
  },
  footer: context [
    #set text(7.5pt, fill: muted)
    #grid(
      columns: (1fr, auto),
      [#data.footer_note],
      [Page #counter(page).display() of #counter(page).final().first()],
    )
  ],
)

#let label(body) = text(7.5pt, weight: "semibold", fill: muted, upper(body))
#let lines(items) = items.filter(x => x != "").join(linebreak())

// ─── Header ─────────────────────────────────────────────────────────────────
#grid(
  columns: (1fr, auto),
  gutter: 12pt,
  [
    #if logo != none {
      image(logo, height: 16mm)
      v(4pt)
    }
    #text(12pt, weight: "bold", data.business.name)
    #if data.business.trading_name != "" and data.business.trading_name != data.business.name [ \ #text(fill: muted, data.business.trading_name) ]
    #set text(8.5pt, fill: muted)
    #if data.business.address_lines.len() > 0 [ \ #lines(data.business.address_lines) ]
    #if data.business.email != "" [ \ #data.business.email ]
    #if data.business.phone != "" [ \ #data.business.phone ]
  ],
  align(right)[
    #text(20pt, weight: "bold", fill: accent, data.title)
    #v(-2pt)
    #text(11pt, weight: "semibold", data.number)
    #v(6pt)
    #set text(8.5pt)
    #grid(
      columns: (auto, auto),
      column-gutter: 10pt,
      row-gutter: 4pt,
      align: (right, right),
      ..data.meta.map(m => (text(fill: muted, m.label), text(weight: "medium", m.value))).flatten()
    )
  ],
)

#v(10pt)
#line(length: 100%, stroke: 0.6pt + rule)
#v(8pt)

// ─── Parties ────────────────────────────────────────────────────────────────
#grid(
  columns: (1fr, 1fr),
  gutter: 16pt,
  [
    #label(data.customer_label)
    #v(2pt)
    #text(weight: "semibold", data.customer.name)
    #set text(8.5pt)
    #if data.customer.business_name != "" and data.customer.business_name != data.customer.name [ \ #data.customer.business_name ]
    #if data.customer.contact_person != "" [ \ Attn: #data.customer.contact_person ]
    #if data.customer.address_lines.len() > 0 [ \ #lines(data.customer.address_lines) ]
    #if data.customer.email != "" [ \ #data.customer.email ]
  ],
  [
    #if data.tax_lines.len() > 0 [
      #label("Tax details")
      #v(2pt)
      #set text(8.5pt)
      #lines(data.tax_lines)
    ]
  ],
)

#v(14pt)

// ─── Lines ──────────────────────────────────────────────────────────────────
#let header-cell(body, align: left) = table.cell(fill: accent.lighten(88%), align: align, text(8pt, weight: "semibold", body))
#table(
  columns: (1fr, auto, auto, auto, auto, auto),
  stroke: (x, y) => if y > 0 { (bottom: 0.5pt + rule) },
  inset: (x: 6pt, y: 6pt),
  align: (left, right, right, right, right, right),
  table.header(
    header-cell[Description],
    header-cell(align: right)[Qty],
    header-cell(align: right)[#data.price_label],
    header-cell(align: right)[Discount],
    header-cell(align: right)[GST],
    header-cell(align: right)[Amount],
  ),
  ..data.lines.map(l => (l.description, l.quantity, l.unit_price, l.discount, l.gst, text(weight: "medium", l.amount))).flatten()
)

// ─── Totals ─────────────────────────────────────────────────────────────────
#v(6pt)
#align(right, block(width: 62mm)[
  #grid(
    columns: (1fr, auto),
    row-gutter: 6pt,
    ..data.totals.map(t => {
      let style = if t.strong { (weight: "bold", size: 11pt) } else { (fill: muted) }
      (text(..style, t.label), align(right, text(..style, t.value)))
    }).flatten()
  )
])

#if data.balance != none [
  #v(8pt)
  #align(right, block(fill: accent.lighten(90%), inset: (x: 10pt, y: 8pt), radius: 4pt, width: 62mm)[
    #grid(
      columns: (1fr, auto),
      text(weight: "bold", data.balance.label), align(right, text(12pt, weight: "bold", fill: accent, data.balance.value)),
    )
  ])
]

// ─── Notes and payment ──────────────────────────────────────────────────────
#v(16pt)
#if data.notes != "" [
  #label("Notes")
  #v(2pt)
  #data.notes
  #v(10pt)
]
#if data.payment_lines.len() > 0 [
  #block(stroke: 0.6pt + rule, radius: 4pt, inset: 10pt, width: 100%)[
    #label("How to pay")
    #v(2pt)
    #lines(data.payment_lines)
  ]
]
#if data.footer != "" [
  #v(10pt)
  #text(8pt, fill: muted, data.footer)
]
