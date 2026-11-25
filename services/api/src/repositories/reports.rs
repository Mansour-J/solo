//! Financial aggregates read from the journal. Every report and dashboard figure comes from here, so
//! totals are always derived the same way. `basis` is `cash` or `accrual`; entries marked `both` count
//! on either basis (docs/architecture/financial-model.md).

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default, Serialize, ToSchema)]
pub struct ProfitTotals {
    pub income: i64,
    pub expenses: i64,
    pub net_profit: i64,
}

pub async fn profit(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<ProfitTotals, sqlx::Error> {
    let (income, expenses): (i64, i64) = sqlx::query_as(
        "SELECT coalesce(sum(l.credit - l.debit) FILTER (WHERE a.account_type = 'income'), 0)::bigint,
                coalesce(sum(l.debit - l.credit) FILTER (WHERE a.account_type = 'expense'), 0)::bigint
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3) AND a.account_type IN ('income', 'expense')",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_one(conn)
    .await?;
    Ok(ProfitTotals {
        income,
        expenses,
        net_profit: income - expenses,
    })
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MonthTotals {
    /// First day of the month.
    pub month: NaiveDate,
    pub income: i64,
    pub expenses: i64,
}

pub async fn monthly(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<Vec<MonthTotals>, sqlx::Error> {
    sqlx::query_as(
        "WITH months AS (
           SELECT generate_series(date_trunc('month', $1::date), date_trunc('month', $2::date), interval '1 month')::date AS month
         )
         SELECT m.month,
                coalesce(sum(l.credit - l.debit) FILTER (WHERE a.account_type = 'income'), 0)::bigint AS income,
                coalesce(sum(l.debit - l.credit) FILTER (WHERE a.account_type = 'expense'), 0)::bigint AS expenses
         FROM months m
         LEFT JOIN journal_entries e
           ON date_trunc('month', e.entry_date)::date = m.month AND e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)
         LEFT JOIN journal_entry_lines l ON l.entry_id = e.id
         LEFT JOIN financial_accounts a ON a.id = l.account_id AND a.account_type IN ('income', 'expense')
         GROUP BY m.month ORDER BY m.month",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_all(conn)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AccountTotal {
    pub account_id: uuid::Uuid,
    pub name: String,
    pub amount: i64,
    /// Not deductible for tax: reports show it as an adjustment.
    #[serde(skip)]
    pub non_deductible: bool,
    /// A direct cost of sales (shown before gross profit).
    #[serde(skip)]
    pub cost_of_sales: bool,
}

/// One account type's (or system account's) balance or movement.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KeyBalance {
    pub account_type: String,
    pub system_key: Option<String>,
    /// Debits less credits.
    pub balance: i64,
}

/// Debits less credits per system account (and per account type for the rest) for entries dated
/// up to `to`, from `from` when given (a movement) or from the start (a balance).
pub async fn balances_by_key(
    conn: &mut PgConnection,
    from: Option<NaiveDate>,
    to: NaiveDate,
    basis: &str,
) -> Result<Vec<KeyBalance>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.account_type, a.system_key, coalesce(sum(l.debit - l.credit), 0)::bigint AS balance
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE e.entry_date <= $2 AND ($1::date IS NULL OR e.entry_date >= $1) AND e.basis IN ('both', $3)
         GROUP BY a.account_type, a.system_key",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_all(conn)
    .await
}

/// Expense totals per expense account (= per category), largest first; zero accounts omitted.
pub async fn expenses_by_account(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<Vec<AccountTotal>, sqlx::Error> {
    totals_by_account(conn, from, to, basis, "expense").await
}

#[derive(Debug, Clone, Copy, Default, Serialize, ToSchema)]
pub struct GstTotals {
    /// GST on sales and income.
    pub collected: i64,
    /// GST claimable on expenses.
    pub claimable: i64,
    /// `collected − claimable`: positive means GST to pay.
    pub net: i64,
}

pub async fn gst(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<GstTotals, sqlx::Error> {
    let (collected, claimable): (i64, i64) = sqlx::query_as(
        "SELECT coalesce(sum(l.credit - l.debit) FILTER (WHERE e.source_type IN ('invoice', 'invoice_payment', 'income')), 0)::bigint,
                coalesce(sum(l.debit - l.credit) FILTER (WHERE e.source_type = 'expense'), 0)::bigint
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE a.system_key = 'gst' AND e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_one(conn)
    .await?;
    Ok(GstTotals {
        collected,
        claimable,
        net: collected - claimable,
    })
}

#[derive(Debug, Clone, Copy, Default, Serialize, ToSchema, sqlx::FromRow)]
pub struct Receivables {
    /// Unpaid amount on issued invoices, today.
    pub outstanding_amount: i64,
    pub outstanding_count: i64,
    pub overdue_amount: i64,
    pub overdue_count: i64,
    /// Issued invoices dated in the period that are fully paid.
    pub paid_count: i64,
    /// Draft invoices waiting to be issued.
    pub draft_count: i64,
}

pub async fn receivables(
    conn: &mut PgConnection,
    today: NaiveDate,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Receivables, sqlx::Error> {
    sqlx::query_as(
        "SELECT coalesce(sum(total - paid) FILTER (WHERE status = 'issued' AND paid < total), 0)::bigint AS outstanding_amount,
                count(*) FILTER (WHERE status = 'issued' AND paid < total) AS outstanding_count,
                coalesce(sum(total - paid) FILTER (WHERE status = 'issued' AND paid < total AND due_date < $1), 0)::bigint AS overdue_amount,
                count(*) FILTER (WHERE status = 'issued' AND paid < total AND due_date < $1) AS overdue_count,
                count(*) FILTER (WHERE status = 'issued' AND paid >= total AND issue_date BETWEEN $2 AND $3) AS paid_count,
                count(*) FILTER (WHERE status = 'draft') AS draft_count
         FROM invoice_balances",
    )
    .bind(today)
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await
}

/// Totals per account of one type (`income` or `expense`), largest first; zero accounts omitted.
pub async fn totals_by_account(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
    account_type: &str,
) -> Result<Vec<AccountTotal>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.id AS account_id, a.name, a.non_deductible, a.cost_of_sales,
                sum(CASE WHEN a.account_type = 'income' THEN l.credit - l.debit ELSE l.debit - l.credit END)::bigint AS amount
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3) AND a.account_type = $4
         GROUP BY a.id, a.name, a.account_type, a.non_deductible, a.cost_of_sales HAVING sum(l.debit - l.credit) <> 0
         ORDER BY amount DESC, a.name",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .bind(account_type)
    .fetch_all(conn)
    .await
}

/// Income from the journal by month, customer and source, for the income summary to group.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IncomeLine {
    pub month: NaiveDate,
    pub customer_id: Option<uuid::Uuid>,
    pub customer_name: Option<String>,
    /// `invoices`, `sales` (income without an invoice) or `other`.
    pub source: String,
    pub amount: i64,
}

pub async fn income_lines(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
    customer_id: Option<uuid::Uuid>,
) -> Result<Vec<IncomeLine>, sqlx::Error> {
    sqlx::query_as(
        "SELECT date_trunc('month', e.entry_date)::date AS month, c.id AS customer_id, c.name AS customer_name,
                CASE WHEN e.source_type IN ('invoice', 'invoice_payment') THEN 'invoices'
                     WHEN ir.income_type = 'sales' THEN 'sales' ELSE 'other' END AS source,
                sum(l.credit - l.debit)::bigint AS amount
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         LEFT JOIN invoices i ON e.source_type = 'invoice' AND i.id = e.source_id
         LEFT JOIN invoice_payments p ON e.source_type = 'invoice_payment' AND p.id = e.source_id
         LEFT JOIN invoices ip ON ip.id = p.invoice_id
         LEFT JOIN income_records ir ON e.source_type = 'income' AND ir.id = e.source_id
         LEFT JOIN customers c ON c.id = coalesce(i.customer_id, ip.customer_id, ir.customer_id)
         WHERE e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3) AND a.account_type = 'income'
           AND ($4::uuid IS NULL OR c.id = $4)
         GROUP BY 1, 2, 3, 4 HAVING sum(l.credit - l.debit) <> 0",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .bind(customer_id)
    .fetch_all(conn)
    .await
}

/// Expense-side journal lines (expenses and mileage claims) by month, account and supplier: expense
/// accounts, GST claimed and the private share (owner drawings).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseLine {
    pub month: NaiveDate,
    pub account_id: uuid::Uuid,
    pub account_name: String,
    /// `expense`, `gst` or `private`.
    pub kind: String,
    pub supplier: String,
    pub amount: i64,
}

/// `account_id` limits it to entries that touch that expense account (a category).
pub async fn expense_lines(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
    account_id: Option<uuid::Uuid>,
) -> Result<Vec<ExpenseLine>, sqlx::Error> {
    sqlx::query_as(
        "SELECT date_trunc('month', e.entry_date)::date AS month, a.id AS account_id, a.name AS account_name,
                CASE WHEN a.account_type = 'expense' THEN 'expense' WHEN a.system_key = 'gst' THEN 'gst' ELSE 'private' END AS kind,
                coalesce(x.supplier, CASE WHEN e.source_type = 'mileage_trip' THEN 'Mileage claims' END, '') AS supplier,
                sum(l.debit - l.credit)::bigint AS amount
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         LEFT JOIN expenses x ON e.source_type = 'expense' AND x.id = e.source_id
         WHERE e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)
           AND (a.account_type = 'expense'
                OR (e.source_type IN ('expense', 'mileage_trip') AND a.system_key IN ('gst', 'owner_drawings')))
           AND ($4::uuid IS NULL OR e.id IN (SELECT l2.entry_id FROM journal_entry_lines l2 WHERE l2.account_id = $4))
         GROUP BY 1, 2, 3, 4, 5 HAVING sum(l.debit - l.credit) <> 0",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .bind(account_id)
    .fetch_all(conn)
    .await
}

/// One journal entry's effect on the GST account, for the records behind the GST summary.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GstEntry {
    pub entry_date: NaiveDate,
    pub description: String,
    pub source_type: String,
    /// GST on sales and income (negative for a reversal).
    pub collected: i64,
    /// GST claimable on expenses and assets bought, and change-of-use adjustments (negative for a
    /// reversal or GST paid back).
    pub claimable: i64,
}

pub async fn gst_entries(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<Vec<GstEntry>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.entry_date, e.description, e.source_type,
                coalesce(sum(l.credit - l.debit) FILTER (WHERE e.source_type IN ('invoice', 'invoice_payment', 'income', 'asset_disposal')), 0)::bigint AS collected,
                coalesce(sum(l.debit - l.credit) FILTER (WHERE e.source_type IN ('expense', 'fixed_asset', 'asset_use_review')), 0)::bigint AS claimable
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE a.system_key = 'gst' AND e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)
         GROUP BY e.id, e.entry_date, e.description, e.source_type, e.created_at
         HAVING sum(l.debit - l.credit) <> 0
         ORDER BY e.entry_date, e.created_at",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_all(conn)
    .await
}

/// An issued invoice with what has been paid (from `invoice_balances`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InvoiceBalance {
    pub invoice_id: uuid::Uuid,
    pub number: Option<String>,
    pub customer_id: uuid::Uuid,
    pub customer_name: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub total: i64,
    pub paid: i64,
}

/// Issued invoices, optionally of one customer and only those still owing.
pub async fn issued_invoices(
    conn: &mut PgConnection,
    customer_id: Option<uuid::Uuid>,
    outstanding_only: bool,
) -> Result<Vec<InvoiceBalance>, sqlx::Error> {
    sqlx::query_as(
        "SELECT b.invoice_id, b.number, b.customer_id, c.name AS customer_name, b.issue_date, b.due_date, b.total, b.paid
         FROM invoice_balances b JOIN customers c ON c.id = b.customer_id
         WHERE b.status = 'issued' AND ($1::uuid IS NULL OR b.customer_id = $1) AND (NOT $2 OR b.paid < b.total)
         ORDER BY b.issue_date, b.number",
    )
    .bind(customer_id)
    .bind(outstanding_only)
    .fetch_all(conn)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CustomerPayment {
    pub payment_date: NaiveDate,
    pub invoice_number: Option<String>,
    pub method: String,
    pub reference: String,
    pub amount: i64,
}

/// Payments (not removed) on a customer's issued invoices.
pub async fn customer_payments(
    conn: &mut PgConnection,
    customer_id: uuid::Uuid,
) -> Result<Vec<CustomerPayment>, sqlx::Error> {
    sqlx::query_as(
        "SELECT p.payment_date, i.number AS invoice_number, p.method, p.reference, p.amount
         FROM invoice_payments p JOIN invoices i ON i.id = p.invoice_id
         WHERE i.customer_id = $1 AND p.voided_at IS NULL AND i.status = 'issued'
         ORDER BY p.payment_date, p.created_at",
    )
    .bind(customer_id)
    .fetch_all(conn)
    .await
}

/// The date of the first journal entry that touches GST, if any.
pub async fn first_gst_entry_date(
    conn: &mut PgConnection,
) -> Result<Option<NaiveDate>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT min(e.entry_date) FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE a.system_key = 'gst'",
    )
    .fetch_one(conn)
    .await
}

/// Totals of the GST paid to IRD and the GST refunds received (cash movements, not voided).
pub async fn gst_settlements(conn: &mut PgConnection) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "SELECT coalesce(sum(amount) FILTER (WHERE kind = 'gst_payment'), 0)::bigint,
                coalesce(sum(amount) FILTER (WHERE kind = 'gst_refund'), 0)::bigint
         FROM cash_movements WHERE voided_at IS NULL",
    )
    .fetch_one(conn)
    .await
}

/// The income tax bands for the tax year starting on `year_start`: the latest set in effect then.
pub async fn income_tax_bands(
    conn: &mut PgConnection,
    year_start: NaiveDate,
) -> Result<Vec<(i64, rust_decimal::Decimal)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT threshold, rate FROM income_tax_rates
         WHERE jurisdiction = 'NZ'
           AND effective_from = (SELECT max(effective_from) FROM income_tax_rates
                                 WHERE jurisdiction = 'NZ' AND effective_from <= $1)
         ORDER BY threshold",
    )
    .bind(year_start)
    .fetch_all(conn)
    .await
}

/// Tax customers withheld from payments (schedular payments) in `[from, to]`.
pub async fn withholding_credits(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT coalesce(sum(l.debit - l.credit), 0)::bigint
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE a.system_key = 'withholding_tax' AND e.source_type = 'invoice_payment'
           AND e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_one(conn)
    .await
}

/// Totals of income tax paid to IRD and income tax refunds received (cash movements, not voided).
pub async fn income_tax_settlements(conn: &mut PgConnection) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "SELECT coalesce(sum(amount) FILTER (WHERE kind = 'income_tax_payment'), 0)::bigint,
                coalesce(sum(amount) FILTER (WHERE kind = 'income_tax_refund'), 0)::bigint
         FROM cash_movements WHERE voided_at IS NULL",
    )
    .fetch_one(conn)
    .await
}

/// The date of the first journal entry, if any.
pub async fn first_entry_date(conn: &mut PgConnection) -> Result<Option<NaiveDate>, sqlx::Error> {
    sqlx::query_scalar("SELECT min(entry_date) FROM journal_entries")
        .fetch_one(conn)
        .await
}

/// The part of expenses that isn't deductible for tax although it's a business cost (an expense's
/// deductible share below its business share, e.g. entertainment at 50%), per expense account; and
/// the same share of the GST claimed on them, under `gst` = true (IRD's yearly GST adjustment).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NonDeductibleShare {
    pub account_id: uuid::Uuid,
    pub name: String,
    pub gst: bool,
    pub amount: i64,
}

pub async fn non_deductible_shares(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<Vec<NonDeductibleShare>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.id AS account_id, a.name, a.system_key IS NOT DISTINCT FROM 'gst' AS gst,
                round(sum((l.debit - l.credit) * (x.business_percentage - x.deductible_percentage)
                          / x.business_percentage))::bigint AS amount
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         JOIN expenses x ON x.id = e.source_id
         WHERE e.source_type = 'expense' AND e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)
           AND x.business_percentage > 0 AND x.deductible_percentage < x.business_percentage
           AND ((a.account_type = 'expense' AND NOT a.non_deductible) OR a.system_key = 'gst')
         GROUP BY a.id, a.name, a.system_key
         HAVING round(sum((l.debit - l.credit) * (x.business_percentage - x.deductible_percentage)
                          / x.business_percentage)) <> 0",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_all(conn)
    .await
}

// ─── GST return boxes ───────────────────────────────────────────────────────

/// One journal entry's parts for a GST return: GST (credit less debit), income (credit less
/// debit), purchases (debits to expenses and fixed assets), and what kind of record it is.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GstBoxEntry {
    pub source_type: String,
    pub gst: i64,
    pub income: i64,
    pub purchases: i64,
    /// An invoice payment that writes the invoice off as a bad debt.
    pub write_off: bool,
    /// Income recorded as exempt from GST (left out of the return's sales).
    pub exempt_income: bool,
    /// For an asset sale: the price received including GST, and the GST on it.
    pub sale_price: Option<i64>,
    pub sale_gst: Option<i64>,
}

pub async fn gst_box_entries(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<Vec<GstBoxEntry>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.source_type,
                coalesce(sum(l.credit - l.debit) FILTER (WHERE a.system_key = 'gst'), 0)::bigint AS gst,
                coalesce(sum(l.credit - l.debit) FILTER (WHERE a.account_type = 'income'), 0)::bigint AS income,
                coalesce(sum(l.debit - l.credit) FILTER (WHERE a.account_type = 'expense' OR a.system_key = 'fixed_assets'), 0)::bigint AS purchases,
                coalesce(bool_or(p.method = 'write_off'), false) AS write_off,
                coalesce(bool_or(ir.gst_treatment = 'exempt'), false) AS exempt_income,
                max(d.sale_price) AS sale_price, max(d.sale_gst) AS sale_gst
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         LEFT JOIN invoice_payments p ON e.source_type = 'invoice_payment' AND p.id = e.source_id
         LEFT JOIN income_records ir ON e.source_type = 'income' AND ir.id = e.source_id
         LEFT JOIN asset_disposals d ON e.source_type = 'asset_disposal' AND d.id = e.source_id
         WHERE e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)
         GROUP BY e.id, e.source_type",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_all(conn)
    .await
}

/// Zero-rated and exempt sales (GST-exclusive amounts) in a period: invoice lines (by issue date on
/// the invoice basis; in proportion to each payment on the payments basis) and income records.
pub async fn untaxed_sales(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    by_payment: bool,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "WITH lines AS (
           SELECT il.tax_treatment, il.net_amount::numeric AS amount
           FROM invoice_items il JOIN invoices i ON i.id = il.invoice_id
           WHERE NOT $3 AND i.status = 'issued' AND i.issue_date BETWEEN $1 AND $2
           UNION ALL
           SELECT il.tax_treatment, il.net_amount::numeric * p.amount / nullif(i.total, 0)
           FROM invoice_payments p JOIN invoices i ON i.id = p.invoice_id JOIN invoice_items il ON il.invoice_id = i.id
           WHERE $3 AND p.voided_at IS NULL AND p.method <> 'write_off' AND p.payment_date BETWEEN $1 AND $2
           UNION ALL
           SELECT r.gst_treatment, r.gross_amount::numeric FROM income_records r
           WHERE r.voided_at IS NULL AND r.income_date BETWEEN $1 AND $2
         )
         SELECT coalesce(round(sum(amount) FILTER (WHERE tax_treatment = 'zero_rated')), 0)::bigint,
                coalesce(round(sum(amount) FILTER (WHERE tax_treatment = 'exempt')), 0)::bigint
         FROM lines",
    )
    .bind(from)
    .bind(to)
    .bind(by_payment)
    .fetch_one(conn)
    .await
}
