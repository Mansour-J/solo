# Changelog

All notable changes to Solo. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and release-please writes each
release's section from the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) merged
since the one before (see `docs/deployment/release-process.md`).

## [0.4.1](https://github.com/Mansour-J/solo/compare/v0.4.0...v0.4.1) (2026-11-25)


### Dependencies

* **desktop:** anyhow 1.0.105, cc 1.11.3, crc32fast 1.5.6 and constant_time_eq 0.6.2 ([2c8280e](https://github.com/Mansour-J/solo/commit/2c8280e8132e7ef68fa266954c3fe3da52c5f551))

## [0.4.0](https://github.com/Mansour-J/solo/releases/tag/v0.4.0) (2026-11-26)

The first release from the public repository. Solo is now free software under the GNU Affero General
Public License v3.0.

### Features

* **updates:** Settings › Updates offers new versions from the public releases, signed and checked before installing. Versions before 0.4.0 can’t update themselves: install this one by hand once.
* **email:** send invoices, quotes and statements through your own email account (Settings › Email, with ready-made settings for Gmail, Outlook, iCloud and Yahoo and a test email).
* **settings:** a System status page checks the database, receipts, PDFs, email and updates, and says what to fix.
* **customers:** email a customer their statement of account as a PDF.
* **invoices:** reminders before and after an invoice is due, sent when the app opens; switch them off for a customer.
* **invoices:** recurring invoices on a schedule.
* **insights:** a weekly email of your notices.
* **reports:** an accountant pack (one zip for last year) and Xero imports of sales invoices and bills.
* **gst:** GST returns for each period with their due dates and what to pay, boxes 5 to 15 ready for myIR, change-of-use adjustments, and a registration reminder.
* **tax:** income tax and provisional tax estimates, with public holidays in IRD’s due dates.
* **year end:** cost of sales and gross profit; asset disposals with depreciation recovered or a loss on sale; low-value assets.
* **expenses:** home office by IRD’s square-metre rate, two-tier kilometre rates, entertainment at 50%, bad debts and secondhand goods.
* **mileage:** return trips and frequent trips.
* **allocations:** spend from an allocation, and allocate payments from before the rules.
* **app:** no sign-in: one person, desktop only; a step-by-step guide for people new to bookkeeping; key dates on the dashboard; sort invoices by any column; export all reports at once; zoom with Ctrl + and Ctrl −.

### Bug fixes

* **expenses:** the expense form previews the figures the app will save, at the GST rate for the date.
* **app:** pages no longer overflow on a phone.
* **updates:** restart cleanly after installing an update, and say why an install failed.
* **security:** updated the `url`, `zip` and `cookie` libraries for published advisories.

## 0.3.0 and earlier

Released from the private repository that preceded this one. Highlights: invoices, quotes, customers,
expenses and income, mileage, allocations, reports with PDF, CSV and Excel exports, insights, GST
returns, income tax and provisional tax, annual accounts (balance sheet, changes in equity, depreciation),
a desktop app with its own database and no sign-in, a beginner’s guide, and checking for updates from
Settings.
