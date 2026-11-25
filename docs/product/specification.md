<!--
The original product specification, kept verbatim as the source of the requirements. It was written
when the product was called "Daikoku"; the product is now Solo (the name, logo and colours come from
branding/brand.json, see ADR 0006). Where this document and the code or docs/ disagree, the decisions
in docs/decisions/ and the roadmap and backlog record what changed and why.
-->

> **Note (2026-11-23):** Solo is now a desktop app for one person with no accounts, sign-in, sharing or
> roles ([ADR 0016](../decisions/0016-one-person-no-sign-in.md)). The requirements below for registration,
> sessions, passwords, email verification, roles and hosted deployment are kept for history only.


# Daikoku — Product & Engineering Specification

## 1. Product overview

Build **Daikoku**, a modern, intuitive bookkeeping and financial-management application inspired by the feature set and workflows of Hnry (https://hnry.co.nz/).

Daikoku is initially intended for New Zealand sole traders, independent contractors, freelancers and self-employed professionals.

The application should make managing business finances straightforward, even for people with little or no accounting knowledge.

### Product philosophy

The application must be:

1. **Easy to use:** Simple workflows, clear information, minimal clicks and excellent UX.
2. **Easy to maintain:** Clean architecture, modular features, consistent coding standards.
3. **Easy to upgrade:** Versioned migrations, backwards-compatible changes and predictable releases.
4. **Easy to install:** Simple Docker deployment, documented configuration and automated setup.
5. **Easy to extend:** New modules, integrations, jurisdictions and functionality should be straightforward to introduce.
6. **Financially reliable:** Accurate calculations, traceable transactions and appropriate audit trails.
7. **Brand independent:** The application must support changing its name, logo, colours and branding without modifying application logic.

### Core product objective

Give an independent professional one place to manage:

- Business income.
- Customers.
- Invoices and quotes.
- Business expenses.
- Receipts.
- Business mileage.
- Income allocations.
- Financial reports.
- Business insights.

The first release must operate primarily through **manual data entry**.

Do not implement automatic bank transaction imports, bank feeds, automatic transaction categorisation, receipt OCR or automatic tax filing in the initial releases.

The user should control what information is entered, how it is classified and how their financial information is presented.

---

# 2. Existing Hermes Hub reference

Before implementing anything, inspect the existing Hermes Hub project and its established development conventions.

Treat Hermes Hub as the reference for architectural decisions, development tooling and implementation patterns.

Investigate and reuse where appropriate:

- Repository and monorepo structure.
- Rust conventions and application architecture.
- Tauri configuration and desktop integration.
- React and TypeScript conventions.
- Mantine components and theming.
- State management.
- API conventions.
- Error handling.
- Logging and observability.
- Docker configuration.
- Environment configuration.
- Database practices.
- GitHub Actions and CI/CD.
- Testing frameworks and conventions.
- Documentation conventions.
- Code quality tooling.
- Release and build processes.

Do not blindly copy Hermes Hub's implementation. Reuse the established decisions that make sense for Daikoku.

**Technology constraints below take precedence if an existing Hermes Hub decision conflicts with the specified Daikoku stack.**

Document which conventions are being reused and any important architectural differences.

---

# 3. Technology stack

Use the following technologies.

| Layer | Technology |
|---|---|
| Programming language | Rust |
| Desktop application | Tauri 2 |
| Frontend | React |
| Language | TypeScript |
| UI framework | Mantine |
| Backend API | Rust |
| API framework | Axum |
| Database | PostgreSQL |
| Database migrations | Flyway |
| Database access | SQLx |
| Frontend build | Vite |
| API documentation | OpenAPI |
| Source control | Private GitHub repository |
| Containerisation | Docker and Docker Compose |
| CI/CD | GitHub Actions |

### Architectural principles

Use a modular monolith initially rather than introducing unnecessary microservices.

Keep the frontend, API, database, authentication and financial domain logic independently structured.

The desktop application must communicate with the backend API. Never expose PostgreSQL credentials or connect directly to PostgreSQL from the React frontend or Tauri client.

Keep the architecture ready for:

- Hosted SaaS deployment.
- Self-hosted deployment.
- Desktop installations.
- Responsive browser access.
- Future native Android and iOS applications.
- Additional integrations.
- Multiple businesses per user.

Prefer established, maintained libraries. Avoid unnecessary dependencies and complicated infrastructure.

---

# 4. Proposed project structure

Start with a clean, maintainable monorepo.

Suggested structure:

```text
daikoku/
│
├── apps/
│   └── client/
│       ├── src/
│       │   ├── app/
│       │   ├── components/
│       │   ├── features/
│       │   │   ├── dashboard/
│       │   │   ├── customers/
│       │   │   ├── invoicing/
│       │   │   ├── expenses/
│       │   │   ├── mileage/
│       │   │   ├── allocations/
│       │   │   ├── reports/
│       │   │   ├── insights/
│       │   │   └── settings/
│       │   ├── hooks/
│       │   ├── layouts/
│       │   ├── lib/
│       │   ├── services/
│       │   ├── theme/
│       │   └── types/
│       └── src-tauri/
│
├── services/
│   └── api/
│       ├── src/
│       │   ├── api/
│       │   ├── auth/
│       │   ├── config/
│       │   ├── domain/
│       │   ├── middleware/
│       │   ├── repositories/
│       │   ├── services/
│       │   ├── jobs/
│       │   └── main.rs
│       └── tests/
│
├── packages/
│   └── api-client/
│
├── database/
│   └── migrations/
│
├── infra/
│   ├── docker/
│   └── scripts/
│
├── docs/
│   ├── architecture/
│   ├── decisions/
│   ├── development/
│   └── deployment/
│
├── .github/
│   └── workflows/
│
├── docker-compose.yml
├── .env.example
├── README.md
└── LICENSE
```

Adjust this structure if Hermes Hub has a better established convention.

Each business module must have a clear separation between:

- UI components.
- API client functionality.
- Domain types.
- Validation.
- Business rules.
- Backend services.
- Database repositories.

Avoid creating an unnecessarily complicated architecture.

---

# 5. Branding and white-label architecture

The initial product name is **Daikoku**.

However, branding must be centrally configurable.

I may want to rename Daikoku in the future without having to search and replace the entire codebase.

Create one central branding configuration.

It should support:

- Application name.
- Application short name.
- Application description.
- Logo.
- Favicon.
- Primary brand colour.
- Secondary brand colour.
- Application icon.
- Email sender name.
- Support email.
- Website URL.
- Application URL.
- Package identifiers.
- Copyright information.

Use environment variables and a central application configuration where appropriate.

Example:

```env
APP_NAME=Daikoku
APP_SHORT_NAME=Daikoku
APP_DESCRIPTION=Simple bookkeeping for independent professionals
APP_URL=http://localhost:5173
APP_SUPPORT_EMAIL=support@example.com
```

Do not hardcode branding strings throughout components.

Keep user-facing application branding separate from technical package names and internal module names.

Changing the central branding configuration should update the application identity consistently.

---

# 6. Authentication and user management

Implement complete authentication, registration and account-management functionality.

Authentication is a foundational feature and must be production quality.

### Registration

- User registration.
- First name.
- Last name.
- Email address.
- Password.
- Password confirmation.
- Terms and privacy acceptance.
- Email verification.
- Resend verification email.
- Appropriate password validation.
- Duplicate email handling.

### Sign-in

- Email and password authentication.
- Remember me.
- Sign out.
- Invalid credential handling.
- Session management.
- Session expiration.
- Secure token handling.

### Password management

- Forgot password.
- Password reset request.
- Secure reset token.
- Password reset confirmation.
- Password expiration of reset tokens.
- Change password when authenticated.
- Revoke existing sessions when appropriate.

### Account management

- User profile.
- Change email address.
- Verify changed email address.
- Change password.
- Manage active sessions.
- Delete account.
- Export user data.
- Manage account preferences.

### Security requirements

- Use Argon2id for password hashing.
- Never store plaintext passwords.
- Secure and single-use password reset tokens.
- Rate-limit authentication endpoints.
- Avoid revealing whether an email exists during password-reset requests.
- Protect against brute-force attacks.
- Implement secure session and refresh-token handling.
- Validate all inputs on the backend.
- Ensure tenant isolation for all business data.
- Never store secrets in frontend source code.

Use appropriate secure storage for Tauri credentials and secure cookies for browser sessions.

Create a clear authentication architecture that works consistently across supported clients.

### Onboarding

After registration, provide a simple onboarding process.

Collect:

- Individual or trading name.
- Business name.
- Business type.
- Business email.
- Phone number.
- Business address.
- New Zealand Business Number (optional).
- GST registration status.
- GST number, where applicable.
- Preferred currency.
- Financial year settings.
- Business logo.

Allow the user to complete optional information later.

A new user should be able to register, create their business profile and start entering financial information without complicated setup.

---

# 7. Dashboard

Build an excellent financial dashboard.

The dashboard should give users an immediate understanding of their business performance.

### Dashboard information

Display:

- Total income.
- Total expenses.
- Net profit.
- Outstanding invoices.
- Overdue invoices.
- Paid invoices.
- Unpaid invoices.
- Monthly income.
- Monthly expenses.
- Current financial year performance.
- Mileage claims.
- Allocation balances.

### Visualisations

Include:

- Income versus expenses.
- Monthly financial performance.
- Expenses by category.
- Revenue over time.
- Outstanding invoices.
- Allocation distribution.

### Dashboard usability

- Configurable date ranges.
- Current month.
- Previous month.
- Current financial year.
- Previous financial year.
- Custom date range.

Allow users to click summary cards and navigate directly to the relevant information.

Provide quick actions:

- Create invoice.
- Add expense.
- Record income.
- Record mileage.
- Add customer.
- View reports.

Never present fake financial figures as actual customer data.

For new users, create helpful empty states explaining how to get started.

---

# 8. Invoicing and quotes

This is one of the primary modules.

Build an intuitive invoicing system inspired by Hnry's functionality.

## 8.1 Customer management

Allow users to create and manage customers.

Customer fields:

- Customer name.
- Business name.
- Contact person.
- Email.
- Phone.
- Billing address.
- Business address.
- NZBN (optional).
- Tax number (optional).
- Notes.
- Payment terms.
- Default currency.
- Active/inactive status.

Support customer search, filtering and customer-specific transaction history.

## 8.2 Invoice creation

Allow users to create invoices manually.

Invoice information:

- Invoice number.
- Customer.
- Issue date.
- Due date.
- Reference.
- Payment terms.
- Currency.
- Invoice description.
- Line items.
- Quantity.
- Unit price.
- GST/tax treatment.
- Discount.
- Subtotal.
- GST amount.
- Total.

Allow multiple line items.

Include configurable invoice numbering and prefixes.

Allow a user to save an invoice as a draft before issuing it.

## 8.3 Invoice management

Invoice statuses:

- Draft.
- Issued.
- Sent.
- Viewed, where delivery tracking supports it.
- Partially paid.
- Paid.
- Overdue.
- Cancelled.

Provide a useful invoice listing with:

- Search.
- Filters.
- Date ranges.
- Customer filters.
- Status filters.
- Sorting.
- Pagination.

## 8.4 Payments

Because Daikoku is manual-entry-first, payment records must be manually entered initially.

Support:

- Full payments.
- Partial payments.
- Payment date.
- Payment method.
- Payment reference.
- Notes.
- Multiple payments against an invoice.
- Outstanding balance calculation.

Allow payments to be edited with a traceable audit history.

Do not imply that money has actually moved through Daikoku. Payment entries represent information entered by the user.

## 8.5 Quotes

Implement a full quotation workflow.

- Create quotes.
- Edit quotes.
- Preview quotes.
- Generate PDF quotes.
- Send quotes by email.
- Mark as accepted.
- Mark as rejected.
- Mark as expired.
- Convert accepted quotes into invoices.

Preserve relevant line items and customer information during conversion.

## 8.6 PDF invoices

Generate professional PDF invoices.

Support:

- Business logo.
- Trading name.
- Business information.
- Customer information.
- Invoice details.
- Line items.
- GST breakdown.
- Total payable.
- Payment details.
- Payment instructions.
- Payment terms.
- Personalised messages.

Provide invoice branding settings.

## 8.7 Email functionality

Implement an email-provider abstraction.

Allow invoices and quotes to be emailed to customers.

Support:

- Email delivery.
- Customisable email content.
- Invoice attachments.
- Quote attachments.
- Delivery status.
- Email history.

Provide a local development email-testing setup.

## 8.8 Future invoicing enhancements

Keep these as future enhancements:

- Recurring invoices.
- Scheduled invoices.
- Automatic overdue reminders.
- Automated payment integrations.
- Credit card payments.
- SMS payment links.

---

# 9. Expense management

Expense management is another primary module.

Users should be able to record and organise business expenses without needing complicated accounting knowledge.

## 9.1 Manual expense entry

Allow users to create an expense manually.

Fields:

- Expense date.
- Supplier/vendor.
- Description.
- Expense category.
- Gross amount.
- Net amount.
- GST amount.
- Currency.
- GST treatment.
- Payment method.
- Payment reference.
- Business/private percentage.
- Deductible percentage.
- Notes.

Provide an expense category management system.

Example categories:

- Advertising.
- Bank fees.
- Insurance.
- Internet.
- Telephone.
- Software.
- Computer equipment.
- Office supplies.
- Professional services.
- Rent.
- Vehicle expenses.
- Travel.
- Meals.
- Subcontractors.
- Utilities.
- Other.

Allow users to create custom categories.

## 9.2 Receipt attachments

Allow receipt attachments.

Supported formats:

- PDF.
- JPEG.
- PNG.
- WebP.

Support multiple attachments where appropriate.

Store attachments securely and associate them with their expense records.

Provide:

- Preview.
- Download.
- Replace.
- Delete.
- Attachment metadata.

Use a storage abstraction so the application can use local storage during development and an S3-compatible provider in production.

Do not implement OCR initially.

## 9.3 Expense classification

Provide clear expense classifications:

- Business expense.
- Personal expense.
- Partially business-related.
- Client-rechargeable expense.

Support manually entered deductible percentages.

Avoid automatically declaring expenses tax-deductible. Classification should be user-controlled and transparent.

## 9.4 Expense management interface

Provide:

- Expense listing.
- Expense detail.
- Expense creation.
- Expense editing.
- Expense deletion with audit history.
- Category filters.
- Supplier filters.
- Date filters.
- Amount filters.
- GST filters.
- Receipt attachment filters.

## 9.5 Expense insights

Display:

- Total expenses.
- Monthly expenses.
- Annual expenses.
- Spending by category.
- Biggest expense categories.
- Expenses over time.
- Business versus personal expenses.
- Client-rechargeable expenses.

## 9.6 Future enhancements

- Receipt OCR.
- Automatic expense categorisation.
- Bank imports.
- Recurring expenses.
- Bank transaction matching.

---

# 10. Mileage management

Create a dedicated mileage module.

Hnry has a separate mileage tracking product, but Daikoku should initially keep mileage inside the main application.

Manual mileage entry is the priority.

## 10.1 Vehicle management

Allow users to register vehicles.

Vehicle details:

- Vehicle name.
- Registration.
- Make.
- Model.
- Year.
- Vehicle type.
- Default mileage rate.
- Notes.

Allow multiple vehicles.

## 10.2 Trip recording

Allow manual trip entry.

Fields:

- Vehicle.
- Trip date.
- Starting location.
- Destination.
- Trip purpose.
- Starting odometer.
- Ending odometer.
- Total distance.
- Business percentage.
- Business distance.
- Applicable mileage rate.
- Calculated claim amount.
- Notes.

Allow users to enter distance directly when odometer readings are unavailable.

## 10.3 Mileage calculations

Calculate mileage claims based on:

Distance × applicable rate × business-use percentage.

Keep mileage rates configurable, with effective dates and historical versions.

Do not hardcode government or IRD rates into business logic.

Allow users to override rates when appropriate and clearly record the rate used for each trip.

## 10.4 Mileage reporting

Display:

- Total kilometres.
- Business kilometres.
- Personal kilometres.
- Total mileage claim.
- Monthly mileage.
- Annual mileage.
- Vehicle breakdown.

Provide date filtering and exports.

## 10.5 Future enhancements

- GPS tracking.
- Automatic trip detection.
- Mobile background tracking.
- Trip classification suggestions.

These should not block the manual mileage functionality.

---

# 11. Allocations

Create an allocations module inspired by Hnry.

The purpose is to help users plan and track where their income should go.

**Important:** Daikoku allocations initially represent bookkeeping and financial planning. They must not claim to transfer money or make actual payments.

## 11.1 Allocation categories

Provide default allocation categories:

- Income tax reserve.
- GST reserve.
- ACC reserve.
- Savings.
- Emergency fund.
- KiwiSaver.
- Investment.
- Mortgage.
- Personal spending.
- Custom allocation.

Allow users to create custom allocation destinations.

## 11.2 Allocation rules

Allow allocations based on:

- Percentage.
- Fixed amount.
- Priority order.
- Income source.
- Customer.
- Payment date.
- Applicable period.

Support:

- Weekly.
- Fortnightly.
- Monthly.
- Custom periods.

## 11.3 Caps and limits

Allow users to set:

- Maximum allocation amounts.
- Monthly caps.
- Weekly caps.
- Minimum allocation balances.
- Allocation targets.

## 11.4 Allocation calculations

When income is manually recorded or an invoice payment is entered, calculate the applicable allocations.

Display:

- Amount received.
- Planned allocations.
- Remaining amount.
- Allocation balances.
- Progress against targets.

Allow allocation results to be reviewed and confirmed.

Keep a traceable allocation history.

Handle corrections and reversed payments without duplicating or corrupting allocation balances.

Do not perform actual bank transfers.

---

# 12. Reports

Build a dedicated reporting module.

Reports should be professional, clear, downloadable and useful for both personal financial management and sharing with an accountant, bank or broker.

## 12.1 Financial reports

Implement:

### Profit and Loss

- Total revenue.
- Other income.
- Total expenses.
- Gross profit.
- Net profit.
- Financial period comparison.

### Income Summary

- Income by month.
- Income by customer.
- Income by source.
- Income by financial year.
- Paid versus outstanding invoices.

### Expense Summary

- Expenses by category.
- Expenses by month.
- Expenses by supplier.
- GST breakdown.
- Business versus personal expenses.

### Statement of Account

Customer-specific account activity:

- Invoices.
- Payments.
- Outstanding invoices.
- Credit balances.
- Payment history.

### Accounts Receivable

- Outstanding invoices.
- Overdue invoices.
- Ageing breakdown.
- Customer balances.

### Financial Statement

- Monthly income.
- Monthly expenses.
- Net position.
- Financial year summary.

### Mileage Report

- Trips.
- Distance.
- Business kilometres.
- Mileage claims.
- Vehicle breakdown.

### Allocation Report

- Allocation rules.
- Allocated amounts.
- Current balances.
- Allocation history.
- Target progress.

### GST Summary

Provide a summary of recorded GST amounts.

This is an informational report, not a tax filing service.

## 12.2 Report filters

Support:

- Current month.
- Previous month.
- Current financial year.
- Previous financial year.
- Custom date range.
- Customer.
- Category.
- Status.

## 12.3 Exports

Support:

- PDF.
- CSV.
- Excel-compatible spreadsheet exports.

Reports must use consistent formatting and accurate calculations.

Make reporting logic reusable so future reports can be added without duplicating calculation code.

---

# 13. Insights

Create a separate insights section.

Reports should show the underlying financial data.

Insights should help users understand what that data means.

Do not simply duplicate the reports interface.

Initially use deterministic calculations and explainable business rules. Do not require an AI provider.

## 13.1 Financial performance

Show:

- Revenue growth.
- Expense growth.
- Profit trends.
- Monthly comparisons.
- Annual comparisons.
- Average monthly revenue.
- Average monthly expenses.

## 13.2 Business health

Show:

- Revenue concentration by customer.
- Top customers.
- Customers with overdue balances.
- Average invoice value.
- Average payment time.
- Unpaid invoice trends.
- Expense-to-revenue ratio.

## 13.3 Spending insights

Identify:

- Biggest expense categories.
- Categories with increasing spending.
- Unusual expense increases.
- Monthly spending patterns.
- Recurring financial commitments.

## 13.4 Cash flow insights

Display:

- Outstanding receivables.
- Expected incoming payments.
- Recorded income.
- Planned allocations.
- Estimated available funds.

Clearly differentiate actual recorded transactions from forecasts and estimates.

## 13.5 Helpful notifications

Examples:

- You have three overdue invoices.
- Your software expenses increased by 15% compared with last month.
- Your highest-spending category this month is software.
- You have not recorded an expense for the current month.
- Your revenue is higher than the previous month.

Do not show insights when there is insufficient data to support them.

---

# 14. Bookkeeping foundation

Treat financial correctness as a core architectural requirement.

Although Daikoku should be very simple to operate, its financial model must be designed carefully.

Use decimal-safe monetary representations. Never use floating-point arithmetic for money.

Use explicit currency codes and configurable currency precision.

Create a consistent financial transaction and accounting-ledger foundation.

Where appropriate, use balanced journal entries and an auditable record of posted transactions.

Separate:

- Draft records.
- Issued invoices.
- Recorded payments.
- Posted expenses.
- Adjustments.
- Reversals.

Maintain clear relationships between invoices, payments, expenses, customers and financial reports.

Financial corrections must not silently corrupt historical figures.

Implement audit history for important financial changes.

Reports must derive their totals consistently from the underlying financial records rather than duplicating independent calculations throughout the application.

Support cash and accrual reporting where appropriate, and clearly label the basis being used.

Make accounting settings configurable rather than embedding assumptions in business logic.

---

# 15. New Zealand localisation

Initially target New Zealand users.

Defaults:

- Currency: NZD.
- Locale: en-NZ.
- Timezone: Pacific/Auckland.
- Date format: DD/MM/YYYY.
- Configurable New Zealand financial year, initially 1 April to 31 March.
- Configurable GST settings and effective rates.

Support GST-registered and non-GST-registered businesses.

Tax rates and mileage rates should be configurable and versioned by effective date.

Do not hardcode tax rules throughout the application.

Do not represent an estimated tax calculation as professional tax advice.

Keep the domain model extensible for other jurisdictions in the future.

---

# 16. Tax filing — explicitly last priority

Tax functionality is important for the long-term direction of Daikoku, but it must be the last major feature group in the initial roadmap.

Do not allow tax filing to delay invoicing, expenses, mileage, allocations or reporting.

Initially provide only:

- GST summaries.
- Financial year summaries.
- Income and expense breakdowns.
- Exportable records for accountants.
- Tax-related configuration.

Future tax backlog:

1. More sophisticated GST calculations.
2. Tax estimates.
3. Provisional tax calculations.
4. Income tax support.
5. ACC-related calculations.
6. Student loan considerations.
7. IRD integration.
8. Tax return preparation.
9. Actual tax filing, subject to appropriate regulatory and integration requirements.

Treat all tax-related integrations as separate modules.

Do not claim Daikoku is an accountant, registered tax agent or authorised tax filing service.

---

# 17. User experience and UI design

UX quality is one of the most important requirements.

The application must feel like a polished commercial SaaS product, not an internal administration tool.

Use Mantine as the primary UI component framework.

Create a consistent design system.

## Design direction

- Modern.
- Minimal.
- Premium.
- Clean.
- Professional.
- Accessible.
- Consistent.
- Responsive.

Use whitespace effectively.

Avoid overcrowded pages, unnecessarily large cards, excessive borders and distracting gradients.

Use a carefully designed typography hierarchy.

Use clear colour semantics for financial statuses.

Examples:

- Paid.
- Unpaid.
- Overdue.
- Draft.
- Completed.
- Pending.

Use a consistent currency formatting system.

## Application layout

Desktop:

- Sidebar navigation.
- Main content area.
- Header with business selector.
- User profile menu.
- Notifications.
- Quick actions.

Mobile:

- Responsive navigation.
- Collapsible sidebar or mobile navigation.
- Touch-friendly controls.
- Appropriately sized forms.
- Responsive tables and cards.

## Navigation

Recommended main navigation:

1. Dashboard.
2. Invoices.
3. Customers.
4. Expenses.
5. Mileage.
6. Allocations.
7. Reports.
8. Insights.

Settings and user management should be accessible from the profile/settings area.

## Interaction requirements

Every important operation should have:

- Appropriate loading feedback.
- Success feedback.
- Error feedback.
- Confirmation where destructive actions are involved.
- Useful validation messages.
- Empty states.
- Skeleton loading states.

Avoid page flickering and unnecessary remounting.

Use optimistic UI updates carefully where financial accuracy is not compromised.

Do not implement inert buttons or decorative controls that appear functional but do nothing.

All forms must have proper validation.

Provide consistent create, edit, view and delete workflows.

Use accessible components and keyboard navigation.

---

# 18. Application settings

Create a central settings area.

Include:

### Business settings

- Business details.
- Trading name.
- Logo.
- Business address.
- Contact information.
- GST status.
- Financial year.
- Currency.

### Invoice settings

- Invoice prefix.
- Starting invoice number.
- Default payment terms.
- Default invoice notes.
- Branding.
- Payment details.

### Expense settings

- Default categories.
- Custom categories.
- Default deductible settings.
- Expense preferences.

### Mileage settings

- Vehicles.
- Mileage rates.
- Default vehicle.

### Allocation settings

- Allocation rules.
- Destinations.
- Targets.
- Caps.

### User preferences

- Profile.
- Language.
- Date formatting.
- Currency formatting.
- Theme.
- Notifications.
- Security.

---

# 19. Database architecture

Use PostgreSQL.

Use Flyway for all database schema migrations.

All changes to database structure must be version-controlled SQL migrations.

Never modify production database schemas manually.

Design for multiple users and multiple businesses.

A user should be able to own more than one business, with each business having its own financial data and settings.

Important domain entities will include:

- users
- user_sessions
- businesses
- business_members
- business_settings
- customers
- invoices
- invoice_items
- invoice_payments
- quotes
- quote_items
- expenses
- expense_categories
- expense_attachments
- vehicles
- mileage_trips
- mileage_rates
- allocation_rules
- allocation_destinations
- allocation_entries
- financial_accounts
- journal_entries
- journal_entry_lines
- audit_logs
- email_logs

This is an initial conceptual model, not a requirement to create every table immediately.

Use proper primary keys, foreign keys, constraints and indexes.

All financial business records must be scoped to the correct business.

Consider PostgreSQL Row-Level Security as an additional defence-in-depth measure.

Implement proper tenant isolation in the application layer regardless.

Do not introduce unnecessary denormalisation.

Use Flyway migration naming conventions:

```text
V1__initial_schema.sql
V2__create_authentication.sql
V3__create_businesses.sql
V4__create_invoicing.sql
V5__create_expenses.sql
```

Never edit an already-applied migration.

Design migrations to be upgrade-friendly.

---

# 20. API architecture

Use Rust and Axum.

Implement a clean REST API.

Suggested API structure:

```text
/api/v1/auth
/api/v1/users
/api/v1/businesses
/api/v1/customers
/api/v1/invoices
/api/v1/quotes
/api/v1/payments
/api/v1/expenses
/api/v1/expense-categories
/api/v1/mileage
/api/v1/allocations
/api/v1/reports
/api/v1/insights
/api/v1/settings
```

Implement:

- Consistent response structures.
- Centralised error handling.
- Request validation.
- Authentication middleware.
- Authorisation middleware.
- Structured logging.
- Pagination.
- Filtering.
- Sorting.
- API versioning.
- Request tracing.

Generate or maintain OpenAPI documentation.

Keep the frontend API client typed and synchronised with backend contracts.

Do not put business calculations inside HTTP handlers.

Keep domain logic inside dedicated services.

---

# 21. Easy installation and deployment

Daikoku should be easy to deploy and maintain.

Provide Docker Compose configuration for:

- PostgreSQL.
- Rust API.
- Web application.
- Flyway migrations.
- Local email testing.

Use persistent storage for database files and uploaded attachments.

Provide health checks.

Use environment-based configuration.

Create a comprehensive `.env.example`.

The initial installation should be as simple as:

```bash
git clone <private-repository>
cd daikoku
cp .env.example .env
docker compose up -d --build
```

Provide an installation/setup script if needed.

Document:

- Initial setup.
- Database configuration.
- Environment variables.
- Email configuration.
- Application URL.
- Backup configuration.
- Restore procedures.
- Upgrade procedures.
- Troubleshooting.

Do not require developers to install unnecessary infrastructure manually.

## Upgrade strategy

Implement a predictable upgrade process.

- Version application releases.
- Run Flyway migrations safely.
- Check backups before production upgrades.
- Maintain migration history.
- Prefer additive, backwards-compatible migrations.
- Document breaking changes.
- Provide database backup and restore scripts.
- Avoid destructive automatic migrations.

The application should be straightforward to upgrade to a newer version without losing user data.

---

# 22. GitHub CI/CD

Use a private GitHub repository.

Implement GitHub Actions workflows.

## Pull request CI

On every pull request:

### Backend

- Rust formatting.
- Clippy.
- Compilation.
- Unit tests.
- API integration tests.
- Database integration tests.

### Frontend

- TypeScript checks.
- ESLint.
- Formatting.
- Frontend build.
- Component tests.

### Database

- Flyway migration validation.
- Migration execution against PostgreSQL.
- Schema integrity checks.

### General

- Dependency security checks.
- Secret scanning.
- Appropriate static analysis.
- Docker build validation.

## Main branch

On successful merge to the main branch:

- Build the API.
- Build the frontend.
- Build Docker images.
- Publish versioned development/staging images where configured.
- Deploy staging where configured.

## Release workflow

When a version tag is created:

- Build production API image.
- Build web application.
- Build desktop application installers.
- Produce release artifacts.
- Generate release notes.
- Publish Docker image to GitHub Container Registry.
- Publish GitHub Release artifacts.

Support release builds for:

- Windows.
- macOS.
- Linux.

Add platform code signing and notarisation when the required credentials are configured.

Use GitHub Actions secrets for credentials.

Never include production secrets in build artifacts.

---

# 23. Testing strategy

Testing must provide confidence in real application behaviour.

Do not rely exclusively on mocked unit tests.

Use a balanced testing strategy.

### Backend

- Domain unit tests.
- Service tests.
- API integration tests.
- Authentication tests.
- Database integration tests.
- Multi-tenant isolation tests.
- Financial calculation tests.

### Frontend

- Component tests.
- Form validation tests.
- Page integration tests.
- Navigation tests.
- Responsive layout checks.

### End-to-end

Use Playwright or another appropriate established framework.

Test complete workflows:

1. Register a new user.
2. Verify email.
3. Sign in.
4. Create a business.
5. Create a customer.
6. Create an invoice.
7. Record a payment.
8. Add an expense.
9. Record a mileage trip.
10. Configure allocations.
11. Generate financial reports.
12. Review business insights.
13. Sign out.
14. Reset password.

Use a real PostgreSQL test database for meaningful integration tests.

Avoid excessively mocking the application layers whose integration is precisely what needs to be tested.

Verify financial totals using realistic data.

Test tenant isolation to ensure users cannot access another business's information.

Test database migrations from a fresh database and an existing supported schema version.

---

# 24. Documentation

Maintain documentation as the application evolves.

Required documents:

- README.
- Product overview.
- Architecture overview.
- Architecture decision records.
- Development setup.
- Installation guide.
- Environment variables reference.
- Database structure.
- Migration guide.
- API documentation.
- Authentication architecture.
- Backup and recovery.
- Release process.
- CI/CD workflow.
- Testing guide.

Keep documentation accurate and update it when architectural decisions change.

---

# 25. Implementation roadmap

Implement Daikoku incrementally.

Do not attempt to build everything in one enormous change.

Prioritise a complete, working product foundation.

## Phase 0 — Project foundation

Highest priority.

- Inspect Hermes Hub.
- Establish repository structure.
- Set up Rust API.
- Set up Tauri and React.
- Configure Mantine.
- Configure PostgreSQL.
- Integrate Flyway.
- Docker Compose.
- Environment configuration.
- Central branding system.
- Initial design system.
- GitHub repository setup.
- CI/CD foundations.

## Phase 1 — Authentication and onboarding

- Registration.
- Sign-in.
- Email verification.
- Forgot password.
- Reset password.
- Sign-out.
- User profile.
- Session handling.
- Business creation.
- Onboarding.

## Phase 2 — Core bookkeeping

- Customer management.
- Invoicing.
- Quotes.
- Invoice PDFs.
- Payment records.
- Income management.
- Expense management.
- Receipt attachments.

This phase must deliver a genuinely usable bookkeeping application.

## Phase 3 — Mileage

- Vehicle management.
- Manual trip entry.
- Mileage calculations.
- Mileage reports.
- Exports.

## Phase 4 — Allocations

- Allocation destinations.
- Allocation rules.
- Percentage allocations.
- Fixed amount allocations.
- Caps.
- Customer-specific allocations.
- Allocation balances.
- Allocation history.

## Phase 5 — Reports

- Profit and loss.
- Income summaries.
- Expense summaries.
- Statements of account.
- Accounts receivable.
- Financial statements.
- Mileage reports.
- Allocation reports.
- GST summaries.
- Exports.

## Phase 6 — Insights

- Business performance.
- Income trends.
- Expense trends.
- Customer insights.
- Invoice insights.
- Financial comparisons.
- Cash flow insights.
- Helpful notifications.

## Phase 7 — Improvements and integrations

- Recurring invoices.
- Invoice reminders.
- Advanced exports.
- Import tools.
- Receipt OCR.
- Bank feeds.
- Payment integrations.
- Other quality-of-life features.

## Phase 8 — Tax functionality

**Lowest implementation priority.**

Only start this phase after the core product is stable and the previous modules are working reliably.

Explore more advanced tax calculations, tax preparation and eventual tax filing integrations.

---

# 26. Development instructions for Claude Code

Work as a senior software architect and experienced full-stack engineer.

Your responsibility is to build a production-quality application, not merely produce a collection of screens.

Follow these instructions:

1. Inspect Hermes Hub and its existing conventions before making architectural decisions.
2. Review the repository and identify existing code that can be reused.
3. Produce a concise architecture proposal and implementation roadmap.
4. Identify important technical risks and resolve foundational decisions early.
5. Implement incrementally using working vertical slices.
6. Keep the application buildable throughout development.
7. Do not introduce unnecessary dependencies.
8. Follow clean Rust and TypeScript conventions.
9. Validate input on both frontend and backend.
10. Keep financial calculations centralised and thoroughly tested.
11. Maintain database migrations as version-controlled SQL.
12. Ensure multi-tenant data isolation.
13. Do not expose secrets.
14. Do not hardcode branding.
15. Do not introduce fake features or nonfunctional UI controls.
16. Do not use placeholder financial data as real information.
17. Implement proper loading, empty, success and error states.
18. Update documentation when functionality or architecture changes.
19. Keep CI green.
20. Prefer maintainability and correctness over unnecessary abstraction.

### Important implementation principle

Prioritise a complete, usable implementation of each module over superficially implementing every module.

For example, invoicing should include customer selection, invoice creation, calculations, saving, editing, PDF generation and payment recording before being considered complete.

Do not mark a feature as implemented simply because its navigation item and page exist.

### Definition of done

A feature is complete when:

- Its UI is implemented.
- Its API is implemented.
- Its database changes are migrated.
- Its validation works.
- Its business logic is correct.
- Its errors are handled.
- Its permissions are enforced.
- Its tests are meaningful.
- Its documentation is updated.

### Final product expectation

Daikoku should feel like a cohesive, premium bookkeeping product designed for independent professionals.

It should be simple enough that a person unfamiliar with accounting can manage their finances, while being technically robust enough to grow into a serious financial-management platform.

Do not overengineer the initial releases, but do not make architectural shortcuts that prevent future growth.

**Build Daikoku as a real product, with excellent UX, reliable financial data, maintainable engineering and an easy upgrade path.**
