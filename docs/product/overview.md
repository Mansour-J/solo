# Product overview

Solo gives a New Zealand sole trader, contractor or freelancer one calm place to run the money side
of their business, without needing to know accounting. It is a desktop app for one person: the books
stay on their computer, and there is no account or sign-in.

How Solo follows IRD's rules for sole traders (GST, withholding tax, expenses), what it doesn't do yet, and the tests for each: [IRD rules](ird-rules.md).

The full requirements are in the original [specification](specification.md) (written under the
working name Daikoku). Its accounts, sign-in, sharing and roles were later removed
([ADR 0016](../decisions/0016-one-person-no-sign-in.md)).

## Principles

1. **Easy to use:** short workflows, plain language, sensible NZ defaults, nothing required that can wait.
2. **Financially reliable:** exact money arithmetic, balanced ledger, audit history, corrections that
   never silently rewrite the past.
3. **Honest:** no fake figures, no buttons that do nothing, no claims to move money or file tax.
   Allocations are a plan; payments are records the user enters.
4. **User-controlled:** manual entry first; the user decides how things are classified.
5. **Yours:** the books stay on your computer; export everything at any time.
6. **Easy to install and change:** one installer, migrations applied on start, central branding.

## Who it's for

Self-employed people in New Zealand: GST-registered or not, financial year from 1 April by default,
NZD, `en-NZ` formatting, `Pacific/Auckland` time. The model is ready for other jurisdictions (tax rates
are data, currencies have explicit precision).

## Modules

Dashboard · Invoices and quotes · Customers · Expenses and receipts · Mileage · Allocations · Reports ·
Insights · Settings. See the [roadmap](roadmap.md) for what is available now.

## Out of scope for the initial releases

Bank feeds and imports, automatic categorisation, receipt OCR, automatic tax filing, payment
processing, recurring invoices and reminders. They are planned as later phases and will be separate
modules.

## Not tax advice

Solo is not an accountant, a registered tax agent or an authorised filing service. GST and financial
year summaries are informational, for the user and their accountant.
