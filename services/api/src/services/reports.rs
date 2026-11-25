//! Reports. Each builds a [`Report`] (sections of typed rows) from `repositories::reports`, the same
//! journal queries the dashboard uses, so a figure is worked out one way everywhere. Statements and
//! ageing list issued invoices and payments (`invoice_balances`), which is what a customer is sent.
//! One export layer (`services::report_export`) renders any report as CSV, Excel or PDF.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::{ReportingBasis, financial_year, financial_year_label};
use crate::domain::money::Currency;
use crate::domain::reports::{
    Bucket, Cell, Column, ColumnKind, Report, ReportPeriod, Section, ageing_bucket, previous_period,
};
use crate::error::{AppError, AppResult};
use crate::repositories::allocations as alloc_repo;
use crate::repositories::businesses::BusinessRow;
use crate::repositories::reports as repo;
use crate::services::businesses::business_today;
use crate::services::invoices::load_business;
use crate::services::mileage;
use crate::state::AppState;

use ColumnKind::{Count, Date, Money, Number, Text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    ProfitAndLoss,
    IncomeSummary,
    ExpenseSummary,
    StatementOfAccount,
    Receivables,
    FinancialStatement,
    Mileage,
    Allocations,
    Gst,
    BalanceSheet,
    EquityChanges,
    DepreciationSchedule,
}

impl ReportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfitAndLoss => "profit_and_loss",
            Self::IncomeSummary => "income_summary",
            Self::ExpenseSummary => "expense_summary",
            Self::StatementOfAccount => "statement_of_account",
            Self::Receivables => "receivables",
            Self::FinancialStatement => "financial_statement",
            Self::Mileage => "mileage",
            Self::Allocations => "allocations",
            Self::Gst => "gst",
            Self::BalanceSheet => "balance_sheet",
            Self::EquityChanges => "equity_changes",
            Self::DepreciationSchedule => "depreciation_schedule",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::ProfitAndLoss => "Profit and loss",
            Self::IncomeSummary => "Income summary",
            Self::ExpenseSummary => "Expense summary",
            Self::StatementOfAccount => "Statement of account",
            Self::Receivables => "Accounts receivable",
            Self::FinancialStatement => "Financial statement",
            Self::Mileage => "Mileage",
            Self::Allocations => "Allocations",
            Self::Gst => "GST summary",
            Self::BalanceSheet => "Balance sheet",
            Self::EquityChanges => "Statement of changes in equity",
            Self::DepreciationSchedule => "Depreciation schedule",
        }
    }
}

/// Filters shared by every report; each report uses the ones that apply to it.
#[derive(Debug, Clone, Default)]
pub struct ReportParams {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub basis: Option<ReportingBasis>,
    pub customer_id: Option<Uuid>,
    /// An expense category.
    pub category_id: Option<Uuid>,
    pub vehicle_id: Option<Uuid>,
    /// An allocation status (allocations report).
    pub status: Option<String>,
}

struct Ctx {
    business: BusinessRow,
    currency: Currency,
    today: NaiveDate,
    from: NaiveDate,
    to: NaiveDate,
    basis: &'static str,
}

impl Ctx {
    fn money(&self, minor: i64) -> String {
        self.currency.format(minor)
    }

    fn report(&self, kind: ReportKind, with_period: bool, with_basis: bool) -> Report {
        Report {
            kind: kind.as_str().into(),
            title: kind.title().into(),
            business_name: if self.business.trading_name.is_empty() {
                self.business.name.clone()
            } else {
                self.business.trading_name.clone()
            },
            currency: self.currency.code.into(),
            period: with_period.then_some(ReportPeriod {
                from: self.from,
                to: self.to,
            }),
            basis: with_basis.then(|| self.basis.to_owned()),
            subtitle: Vec::new(),
            sections: Vec::new(),
            notes: Vec::new(),
            generated_on: self.today,
        }
    }
}

fn check_period(from: NaiveDate, to: NaiveDate) -> AppResult<()> {
    if to < from {
        return Err(AppError::validation(
            "to",
            "The end date can’t be before the start date.",
        ));
    }
    if (to - from).num_days() > 366 * 10 {
        return Err(AppError::validation(
            "from",
            "Choose a period of up to ten years.",
        ));
    }
    Ok(())
}

async fn context(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    params: &ReportParams,
) -> AppResult<Ctx> {
    let business = load_business(conn, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let (fy_start, fy_end) = financial_year(
        today,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let from = params.from.unwrap_or(fy_start);
    let to = params.to.unwrap_or(fy_end);
    check_period(from, to)?;
    let currency = Currency::from_code(&business.currency).ok_or_else(|| {
        AppError::internal(anyhow::anyhow!("unknown currency {}", business.currency))
    })?;
    let basis = match params.basis.unwrap_or(business.reporting_basis) {
        ReportingBasis::Cash => "cash",
        ReportingBasis::Accrual => "accrual",
    };
    Ok(Ctx {
        business,
        currency,
        today,
        from,
        to,
        basis,
    })
}

/// The reports "Export all" includes, in the order the reports page lists them. The statement of
/// account is left out: it is for one customer at a time.
pub const EXPORT_ALL: [ReportKind; 11] = [
    ReportKind::ProfitAndLoss,
    ReportKind::BalanceSheet,
    ReportKind::EquityChanges,
    ReportKind::DepreciationSchedule,
    ReportKind::IncomeSummary,
    ReportKind::ExpenseSummary,
    ReportKind::Receivables,
    ReportKind::FinancialStatement,
    ReportKind::Gst,
    ReportKind::Mileage,
    ReportKind::Allocations,
];

/// Every report in [`EXPORT_ALL`] for one period and basis, without the per-report filters
/// (customer, category, vehicle, status), so each covers the whole business.
pub async fn build_all(
    state: &AppState,
    ctx: &BusinessContext,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    basis: Option<ReportingBasis>,
) -> AppResult<Vec<Report>> {
    let params = ReportParams {
        from,
        to,
        basis,
        customer_id: None,
        category_id: None,
        vehicle_id: None,
        status: None,
    };
    let mut reports = Vec::with_capacity(EXPORT_ALL.len());
    for kind in EXPORT_ALL {
        reports.push(build(state, ctx, kind, &params).await?);
    }
    Ok(reports)
}

/// Builds a report for the business. Reading only.
pub async fn build(
    state: &AppState,
    ctx: &BusinessContext,
    kind: ReportKind,
    params: &ReportParams,
) -> AppResult<Report> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let c = context(&mut tx, ctx, params).await?;
    let report = match kind {
        ReportKind::ProfitAndLoss => profit_and_loss(&mut tx, &c).await?,
        ReportKind::IncomeSummary => income_summary(&mut tx, &c, params).await?,
        ReportKind::ExpenseSummary => expense_summary(&mut tx, &c, params).await?,
        ReportKind::StatementOfAccount => statement(&mut tx, &c, params).await?,
        ReportKind::Receivables => receivables(&mut tx, &c, params).await?,
        ReportKind::FinancialStatement => financial_statement(&mut tx, &c).await?,
        ReportKind::Allocations => allocations(&mut tx, &c, params).await?,
        ReportKind::Gst => gst(&mut tx, &c).await?,
        ReportKind::BalanceSheet => balance_sheet(&mut tx, &c).await?,
        ReportKind::EquityChanges => equity_changes(&mut tx, &c).await?,
        ReportKind::DepreciationSchedule => depreciation_schedule(&mut tx, &c).await?,
        ReportKind::Mileage => {
            tx.commit().await?;
            return mileage_report(state, ctx, c, params).await;
        }
    };
    tx.commit().await?;
    Ok(report)
}

fn month_label(month: NaiveDate) -> String {
    month.format("%b %Y").to_string()
}

fn months_between(from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    let mut months = Vec::new();
    let mut m = from.with_day0(0).unwrap_or(from);
    while m <= to {
        months.push(m);
        m = m
            .checked_add_months(chrono::Months::new(1))
            .unwrap_or(to + chrono::Duration::days(1));
    }
    months
}

use chrono::Datelike;

// ─── Profit and loss ────────────────────────────────────────────────────────

async fn profit_and_loss(conn: &mut PgConnection, c: &Ctx) -> AppResult<Report> {
    let (prev_from, prev_to) = previous_period(c.from, c.to);
    let columns = || {
        vec![
            Column::new("Account", Text),
            Column::new("This period", Money),
            Column::new("Previous period", Money),
            Column::new("Change", Money),
        ]
    };
    let mut report = c.report(ReportKind::ProfitAndLoss, true, true);
    report.subtitle.push(format!(
        "Compared with {} to {}",
        prev_from.format("%-d %b %Y"),
        prev_to.format("%-d %b %Y")
    ));
    // (name, this period, previous period, not deductible, cost of sales) per account.
    type Line = (String, i64, i64, bool, bool);
    let mut by_type: Vec<Vec<Line>> = Vec::new();
    // Business costs that aren't fully deductible (an expense's deductible share below its business
    // share, e.g. entertainment at 50%): the non-deductible part moves to the adjustments.
    let shares_now = repo::non_deductible_shares(conn, c.from, c.to, c.basis).await?;
    let shares_before = repo::non_deductible_shares(conn, prev_from, prev_to, c.basis).await?;
    let mut partly: BTreeMap<Uuid, Line> = BTreeMap::new();
    for (share, slot) in shares_now
        .iter()
        .map(|s| (s, 1))
        .chain(shares_before.iter().map(|s| (s, 2)))
        .filter(|(s, _)| !s.gst)
    {
        let entry = partly.entry(share.account_id).or_insert((
            format!("{} (non-deductible share)", share.name),
            0,
            0,
            true,
            false,
        ));
        if slot == 1 {
            entry.1 = share.amount
        } else {
            entry.2 = share.amount
        }
    }
    for account_type in ["income", "expense"] {
        let now = repo::totals_by_account(conn, c.from, c.to, c.basis, account_type).await?;
        let before =
            repo::totals_by_account(conn, prev_from, prev_to, c.basis, account_type).await?;
        let mut accounts: BTreeMap<Uuid, Line> = BTreeMap::new();
        for (a, slot) in now
            .iter()
            .map(|a| (a, 1))
            .chain(before.iter().map(|a| (a, 2)))
        {
            let entry = accounts.entry(a.account_id).or_insert((
                a.name.clone(),
                0,
                0,
                a.non_deductible,
                a.cost_of_sales,
            ));
            if slot == 1 {
                entry.1 = a.amount
            } else {
                entry.2 = a.amount
            }
        }
        if account_type == "expense" {
            for (id, share) in &partly {
                if let Some(account) = accounts.get_mut(id) {
                    account.1 -= share.1;
                    account.2 -= share.2;
                }
            }
        }
        let mut rows: Vec<Line> = accounts.into_values().collect();
        if account_type == "expense" {
            rows.extend(partly.values().cloned());
        }
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
        by_type.push(rows);
    }
    let section_of = |title: &str, total_label: &str, rows: &[&Line]| -> (Section, [i64; 2]) {
        let mut section = Section::new(title, columns());
        for (name, current, previous, _, _) in rows {
            section.row(vec![
                Cell::text(name.clone()),
                Cell::from(*current),
                Cell::from(*previous),
                Cell::from(current - previous),
            ]);
        }
        let current: i64 = rows.iter().map(|r| r.1).sum();
        let previous: i64 = rows.iter().map(|r| r.2).sum();
        section.totals = Some(vec![
            Cell::text(total_label),
            Cell::from(current),
            Cell::from(previous),
            Cell::from(current - previous),
        ]);
        (section, [current, previous])
    };
    let income_rows: Vec<&Line> = by_type[0].iter().collect();
    let cost_rows: Vec<&Line> = by_type[1].iter().filter(|r| !r.3 && r.4).collect();
    let deductible: Vec<&Line> = by_type[1].iter().filter(|r| !r.3 && !r.4).collect();
    let adjustments: Vec<&Line> = by_type[1].iter().filter(|r| r.3).collect();
    let sales = by_type[0]
        .iter()
        .find(|r| r.0 == "Sales")
        .map_or([0, 0], |r| [r.1, r.2]);
    let (income_section, income) = section_of("Income", "Total income", &income_rows);
    let (expense_section, expenses) = section_of("Expenses", "Total expenses", &deductible);
    report.sections.push(income_section);
    let mut cost = [0, 0];
    if !cost_rows.is_empty() {
        let (section, totals) = section_of("Cost of sales", "Total cost of sales", &cost_rows);
        report
            .sections
            .push(section.describe("Direct costs of what was sold, such as materials and stock."));
        cost = totals;
    }
    report.sections.push(expense_section);
    let mut adjusted = [0, 0];
    if !adjustments.is_empty() {
        let (section, totals) =
            section_of("Adjustments", "Total non-deductible expenses", &adjustments);
        report.sections.push(section.describe(
            "Non-deductible expenses: costs of the business that can’t be claimed for tax.",
        ));
        adjusted = totals;
    }
    let row = |label: &str, current: i64, previous: i64| {
        vec![
            Cell::text(label),
            Cell::from(current),
            Cell::from(previous),
            Cell::from(current - previous),
        ]
    };
    let before_adjustments = [
        income[0] - cost[0] - expenses[0],
        income[1] - cost[1] - expenses[1],
    ];
    let mut summary = Section::new("Profit", columns());
    summary.row(row("Gross profit", sales[0] - cost[0], sales[1] - cost[1]));
    if !adjustments.is_empty() {
        summary.row(row(
            "Net profit before adjustments",
            before_adjustments[0],
            before_adjustments[1],
        ));
    }
    summary.row(row(
        "Net profit",
        before_adjustments[0] - adjusted[0],
        before_adjustments[1] - adjusted[1],
    ));
    report.sections.push(summary);
    report.notes.push(if cost_rows.is_empty() {
        "Gross profit is revenue from sales: no category is marked as cost of sales, so it is before any direct costs.".into()
    } else {
        "Gross profit is revenue from sales less cost of sales.".into()
    });
    report.notes.push(
        "Mileage claims and depreciation are included in expenses. Owner drawings, the private share of expenses and taxes paid are not.".into(),
    );
    Ok(report)
}

// ─── Balance sheet and equity ───────────────────────────────────────────────

/// Balances that make up a balance sheet at one date (debits less credits, signs turned so assets,
/// liabilities and equity read as on a balance sheet).
#[derive(Debug, Clone, Copy, Default)]
struct Position {
    bank: i64,
    receivables: i64,
    fixed_assets: i64,
    /// Owed to IRD (negative when a refund is due).
    gst_payable: i64,
    current_earnings: i64,
    retained_earnings: i64,
    drawings: i64,
    funds_introduced: i64,
    personal_accounts: i64,
    withholding_tax: i64,
    income_tax: i64,
}

impl Position {
    fn net_assets(&self) -> i64 {
        self.bank + self.receivables + self.fixed_assets - self.gst_payable
    }

    fn equity(&self) -> i64 {
        self.current_earnings
            + self.retained_earnings
            + self.drawings
            + self.funds_introduced
            + self.personal_accounts
            + self.withholding_tax
            + self.income_tax
    }
}

fn key_balance(rows: &[repo::KeyBalance], key: &str) -> i64 {
    rows.iter()
        .filter(|r| r.system_key.as_deref() == Some(key))
        .map(|r| r.balance)
        .sum()
}

fn type_balance(rows: &[repo::KeyBalance], account_type: &str) -> i64 {
    rows.iter()
        .filter(|r| r.account_type == account_type)
        .map(|r| r.balance)
        .sum()
}

/// The position at the end of `date`; profit since `year_start` is this year's earnings.
async fn position(
    conn: &mut PgConnection,
    c: &Ctx,
    date: NaiveDate,
    year_start: NaiveDate,
) -> AppResult<Position> {
    let all = repo::balances_by_key(conn, None, date, c.basis).await?;
    let before_year =
        repo::balances_by_key(conn, None, year_start - chrono::Duration::days(1), c.basis).await?;
    // Income is a credit (negative) balance, expenses a debit: profit = −(income + expenses).
    let profit =
        |rows: &[repo::KeyBalance]| -(type_balance(rows, "income") + type_balance(rows, "expense"));
    let earlier_profit = profit(&before_year);
    Ok(Position {
        bank: key_balance(&all, "bank"),
        receivables: key_balance(&all, "accounts_receivable"),
        fixed_assets: key_balance(&all, "fixed_assets"),
        gst_payable: -key_balance(&all, "gst"),
        current_earnings: profit(&all) - earlier_profit,
        retained_earnings: -key_balance(&all, "retained_earnings") + earlier_profit,
        drawings: -key_balance(&all, "owner_drawings"),
        funds_introduced: -key_balance(&all, "owner_funds"),
        personal_accounts: -key_balance(&all, "personal_accounts"),
        withholding_tax: -key_balance(&all, "withholding_tax"),
        income_tax: -key_balance(&all, "income_tax"),
    })
}

fn fy_start_of(c: &Ctx, date: NaiveDate) -> NaiveDate {
    financial_year(
        date,
        c.business.financial_year_start_month as u32,
        c.business.financial_year_start_day as u32,
    )
    .0
}

async fn balance_sheet(conn: &mut PgConnection, c: &Ctx) -> AppResult<Report> {
    let as_at = c.to;
    let previous = fy_start_of(c, as_at) - chrono::Duration::days(1);
    let now = position(conn, c, as_at, fy_start_of(c, as_at)).await?;
    let then = position(conn, c, previous, fy_start_of(c, previous)).await?;
    let mut report = c.report(ReportKind::BalanceSheet, false, true);
    report
        .subtitle
        .push(format!("As at {}", as_at.format("%-d %B %Y")));
    let label = |d: NaiveDate| d.format("%-d %b %Y").to_string();
    let columns = || {
        vec![
            Column::new("", Text),
            Column::new(&label(as_at), Money),
            Column::new(&label(previous), Money),
        ]
    };
    let line = |name: &str, a: i64, b: i64| vec![Cell::text(name), Cell::from(a), Cell::from(b)];
    let mut assets = Section::new("Assets", columns());
    assets.row(line("Business bank account", now.bank, then.bank));
    if now.receivables != 0 || then.receivables != 0 {
        assets.row(line(
            "Accounts receivable",
            now.receivables,
            then.receivables,
        ));
    }
    if now.gst_payable < 0 || then.gst_payable < 0 {
        assets.row(line(
            "GST refund due",
            (-now.gst_payable).max(0),
            (-then.gst_payable).max(0),
        ));
    }
    assets.row(line(
        "Property, plant and equipment",
        now.fixed_assets,
        then.fixed_assets,
    ));
    let total_assets =
        |p: &Position| p.bank + p.receivables + p.fixed_assets + (-p.gst_payable).max(0);
    assets.totals = Some(line(
        "Total assets",
        total_assets(&now),
        total_assets(&then),
    ));
    let mut liabilities = Section::new("Liabilities", columns());
    liabilities.row(line(
        "GST payable",
        now.gst_payable.max(0),
        then.gst_payable.max(0),
    ));
    liabilities.totals = Some(line(
        "Total liabilities",
        now.gst_payable.max(0),
        then.gst_payable.max(0),
    ));
    let mut net = Section::new("Net assets", columns());
    net.totals = Some(line("Net assets", now.net_assets(), then.net_assets()));
    let mut equity = Section::new("Equity", columns());
    for (name, a, b) in [
        (
            "Current year earnings",
            now.current_earnings,
            then.current_earnings,
        ),
        ("Drawings", now.drawings, then.drawings),
        (
            "Owner funds introduced",
            now.funds_introduced,
            then.funds_introduced,
        ),
        (
            "Personal accounts",
            now.personal_accounts,
            then.personal_accounts,
        ),
        (
            "Retained earnings",
            now.retained_earnings,
            then.retained_earnings,
        ),
        (
            "Withholding tax paid",
            now.withholding_tax,
            then.withholding_tax,
        ),
        ("Income tax paid", now.income_tax, then.income_tax),
    ] {
        if a != 0 || b != 0 {
            equity.row(line(name, a, b));
        }
    }
    equity.totals = Some(line("Total equity", now.equity(), then.equity()));
    report.sections.extend([assets, liabilities, net, equity]);
    report.notes.push(
        "From the journal. Withholding tax and income tax paid are the owner's tax payments, shown in equity as an accountant would.".into(),
    );
    Ok(report)
}

async fn equity_changes(conn: &mut PgConnection, c: &Ctx) -> AppResult<Report> {
    let (prev_from, prev_to) = previous_period(c.from, c.to);
    let mut report = c.report(ReportKind::EquityChanges, true, true);
    let label = |from: NaiveDate, to: NaiveDate| {
        format!("{} – {}", from.format("%b %Y"), to.format("%b %Y"))
    };
    let columns = || {
        vec![
            Column::new("", Text),
            Column::new(&label(c.from, c.to), Money),
            Column::new(&label(prev_from, prev_to), Money),
        ]
    };
    // Per period: the opening equity and each movement as it changes equity (+ increases it).
    let mut periods = Vec::new();
    for (from, to) in [(c.from, c.to), (prev_from, prev_to)] {
        let day_before = from - chrono::Duration::days(1);
        let opening = position(conn, c, day_before, fy_start_of(c, day_before)).await?;
        let moved = repo::balances_by_key(conn, Some(from), to, c.basis).await?;
        let change = |key: &str| -key_balance(&moved, key);
        periods.push(EquityMovements {
            opening: opening.equity(),
            profit: -(type_balance(&moved, "income") + type_balance(&moved, "expense")),
            funds_introduced: change("owner_funds"),
            drawings: change("owner_drawings"),
            withholding_tax: change("withholding_tax"),
            income_tax: change("income_tax"),
            other: change("personal_accounts") + change("retained_earnings"),
        });
    }
    let (now, before) = (periods[0], periods[1]);
    let row = |name: &str, a: i64, b: i64| vec![Cell::text(name), Cell::from(a), Cell::from(b)];

    let mut opening = Section::new("Equity", columns());
    opening.row(row("Opening balance", now.opening, before.opening));
    // An accountant lists what added to equity and what reduced it, each as a positive amount.
    let mut increases = Section::new("Increases", columns());
    let mut decreases = Section::new("Decreases", columns());
    for (increase, decrease, pick) in EquityMovements::LINES {
        let (a, b) = (pick(&now), pick(&before));
        if a > 0 || b > 0 || (increase == "Profit for the period" && a == 0 && b == 0) {
            increases.row(row(increase, a.max(0), b.max(0)));
        }
        if a < 0 || b < 0 {
            decreases.row(row(decrease, (-a).max(0), (-b).max(0)));
        }
    }
    increases.totals = Some(row("Total increases", now.increases(), before.increases()));
    decreases.totals = Some(row("Total decreases", now.decreases(), before.decreases()));
    let mut total = Section::new("Total", columns());
    total.totals = Some(row("Total equity", now.closing(), before.closing()));
    report
        .sections
        .extend([opening, increases, decreases, total]);
    Ok(report)
}

/// What changed the owner's equity in a period, each signed as it affects equity.
#[derive(Debug, Clone, Copy)]
struct EquityMovements {
    opening: i64,
    profit: i64,
    funds_introduced: i64,
    drawings: i64,
    withholding_tax: i64,
    income_tax: i64,
    other: i64,
}

type Pick = fn(&EquityMovements) -> i64;

impl EquityMovements {
    /// (name as an increase, name as a decrease, value).
    const LINES: [(&'static str, &'static str, Pick); 6] = [
        ("Profit for the period", "Loss for the period", |m| m.profit),
        ("Owner funds introduced", "Owner funds withdrawn", |m| {
            m.funds_introduced
        }),
        ("Drawings returned", "Drawings", |m| m.drawings),
        ("Withholding tax refunded", "Withholding tax paid", |m| {
            m.withholding_tax
        }),
        ("Income tax refunded", "Income tax paid", |m| m.income_tax),
        ("Other increases", "Other decreases", |m| m.other),
    ];

    fn values(&self) -> impl Iterator<Item = i64> + '_ {
        Self::LINES.iter().map(move |(_, _, pick)| pick(self))
    }

    fn increases(&self) -> i64 {
        self.values().filter(|v| *v > 0).sum()
    }

    fn decreases(&self) -> i64 {
        -self.values().filter(|v| *v < 0).sum::<i64>()
    }

    fn closing(&self) -> i64 {
        self.opening + self.increases() - self.decreases()
    }
}

async fn depreciation_schedule(conn: &mut PgConnection, c: &Ctx) -> AppResult<Report> {
    use crate::repositories::annual as annual_repo;
    let year_end = financial_year(
        c.to,
        c.business.financial_year_start_month as u32,
        c.business.financial_year_start_day as u32,
    )
    .1;
    let mut report = c.report(ReportKind::DepreciationSchedule, false, false);
    report.subtitle.push(format!(
        "For the year ended {}",
        year_end.format("%-d %B %Y")
    ));
    let assets = annual_repo::list_assets(conn).await?;
    let entries: Vec<annual_repo::DepreciationRow> = annual_repo::depreciation_entries(conn, None)
        .await?
        .into_iter()
        .filter(|e| e.year_end == year_end)
        .collect();
    let columns = || {
        vec![
            Column::new("Name", Text),
            Column::new("Cost", Money),
            Column::new("Opening value", Money),
            Column::new("Purchases", Money),
            Column::new("Disposals", Money),
            Column::new("Depreciation", Money),
            Column::new("Closing value", Money),
            Column::new("Private use", Money),
        ]
    };
    // (asset, opening, purchases, disposals, depreciation, closing, private use)
    type Row<'a> = (&'a annual_repo::AssetRow, [i64; 6]);
    let mut groups: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for e in &entries {
        if let Some(a) = assets.iter().find(|a| a.id == e.asset_id) {
            groups.entry(a.asset_group.clone()).or_default().push((
                a,
                [
                    e.opening_value,
                    e.purchases,
                    0,
                    e.depreciation,
                    e.closing_value,
                    e.private_amount,
                ],
            ));
        }
    }
    // Assets sold this year: no depreciation; the value at the start of the year goes out as a disposal.
    let year_start = financial_year(
        year_end,
        c.business.financial_year_start_month as u32,
        c.business.financial_year_start_day as u32,
    )
    .0;
    let disposals = annual_repo::list_disposals(conn).await?;
    for d in disposals
        .iter()
        .filter(|d| d.disposed_on >= year_start && d.disposed_on <= year_end)
    {
        if let Some(a) = assets.iter().find(|a| a.id == d.asset_id) {
            let bought_this_year = a.acquired_on >= year_start && a.opening_value.is_none();
            let (opening, purchases) = if bought_this_year {
                (0, d.book_value)
            } else {
                (d.book_value, 0)
            };
            groups
                .entry(a.asset_group.clone())
                .or_default()
                .push((a, [opening, purchases, d.book_value, 0, 0, 0]));
        }
    }
    let total = |items: &[Row], label: String| {
        let sum = |i: usize| items.iter().map(|(_, v)| v[i]).sum::<i64>();
        vec![
            Cell::text(label),
            Cell::from(items.iter().map(|(a, _)| a.cost).sum::<i64>()),
            Cell::from(sum(0)),
            Cell::from(sum(1)),
            Cell::from(sum(2)),
            Cell::from(sum(3)),
            Cell::from(sum(4)),
            Cell::from(sum(5)),
        ]
    };
    let mut all = Vec::new();
    for (group, mut items) in groups {
        items.sort_by(|a, b| a.0.name.cmp(&b.0.name));
        let mut section = Section::new(&group, columns());
        for (a, v) in &items {
            section.row(vec![
                Cell::text(a.name.clone()),
                Cell::from(a.cost),
                Cell::from(v[0]),
                Cell::from(v[1]),
                Cell::from(v[2]),
                Cell::from(v[3]),
                Cell::from(v[4]),
                Cell::from(v[5]),
            ]);
        }
        section.totals = Some(total(&items, format!("Total {}", group.to_lowercase())));
        all.extend(items);
        report.sections.push(section);
    }
    let mut summary = Section::new("Total", columns());
    summary.totals = Some(total(&all, "Total".into()));
    report.sections.push(summary);
    if entries.is_empty() && all.is_empty() {
        report
            .notes
            .push("No depreciation has been posted for this year yet. Post it from Assets.".into());
    }
    report.notes.push(
        "Rates and methods are the ones entered for each asset. An asset sold in the year has no depreciation for it: its value at the start of the year goes out under disposals."
            .into(),
    );
    Ok(report)
}

// ─── Income summary ─────────────────────────────────────────────────────────

fn source_label(source: &str) -> &'static str {
    match source {
        "invoices" => "Invoices",
        "sales" => "Sales without an invoice",
        _ => "Other income",
    }
}

async fn income_summary(
    conn: &mut PgConnection,
    c: &Ctx,
    params: &ReportParams,
) -> AppResult<Report> {
    let mut report = c.report(ReportKind::IncomeSummary, true, true);
    if let Some(name) = customer_name(conn, params.customer_id).await? {
        report.subtitle.push(format!("Customer: {name}"));
    }
    let lines = repo::income_lines(conn, c.from, c.to, c.basis, params.customer_id).await?;
    let total: i64 = lines.iter().map(|l| l.amount).sum();
    let amount_columns = |first: &str| vec![Column::new(first, Text), Column::new("Income", Money)];

    let mut by_month: BTreeMap<NaiveDate, i64> = months_between(c.from, c.to)
        .into_iter()
        .map(|m| (m, 0))
        .collect();
    for l in &lines {
        *by_month.entry(l.month).or_default() += l.amount;
    }
    let mut section = Section::new("By month", amount_columns("Month"));
    for (month, amount) in &by_month {
        section.row(vec![Cell::text(month_label(*month)), Cell::from(*amount)]);
    }
    section.totals = Some(vec![Cell::text("Total"), Cell::from(total)]);
    report.sections.push(section);

    let mut by_customer: BTreeMap<String, i64> = BTreeMap::new();
    for l in &lines {
        *by_customer
            .entry(
                l.customer_name
                    .clone()
                    .unwrap_or_else(|| "No customer".into()),
            )
            .or_default() += l.amount;
    }
    let mut customers: Vec<(String, i64)> = by_customer.into_iter().collect();
    customers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut section = Section::new("By customer", amount_columns("Customer"));
    for (name, amount) in customers {
        section.row(vec![Cell::text(name), Cell::from(amount)]);
    }
    section.totals = Some(vec![Cell::text("Total"), Cell::from(total)]);
    report.sections.push(section);

    let mut by_source: BTreeMap<&str, i64> = BTreeMap::new();
    for l in &lines {
        *by_source.entry(source_label(&l.source)).or_default() += l.amount;
    }
    let mut section = Section::new("By source", amount_columns("Source"));
    for (name, amount) in by_source {
        section.row(vec![Cell::text(name), Cell::from(amount)]);
    }
    section.totals = Some(vec![Cell::text("Total"), Cell::from(total)]);
    report.sections.push(section);

    // Financial years the period touches.
    let (start_month, start_day) = (
        c.business.financial_year_start_month as u32,
        c.business.financial_year_start_day as u32,
    );
    let mut by_year: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for l in &lines {
        let (_, end) = financial_year(l.month, start_month, start_day);
        *by_year.entry(end).or_default() += l.amount;
    }
    let mut section = Section::new("By financial year", amount_columns("Financial year"));
    for (end, amount) in by_year {
        section.row(vec![
            Cell::text(financial_year_label(end)),
            Cell::from(amount),
        ]);
    }
    report.sections.push(section);

    // Invoices issued in the period: paid and still owing (as at today).
    let invoices: Vec<repo::InvoiceBalance> =
        repo::issued_invoices(conn, params.customer_id, false)
            .await?
            .into_iter()
            .filter(|i| i.issue_date >= c.from && i.issue_date <= c.to)
            .collect();
    let mut section = Section::new(
        "Invoices issued in the period",
        vec![
            Column::new("", Text),
            Column::new("Invoices", Count),
            Column::new("Invoiced", Money),
            Column::new("Paid", Money),
            Column::new("Outstanding", Money),
        ],
    )
    .describe("Paid and outstanding as at today.");
    let paid: Vec<_> = invoices.iter().filter(|i| i.paid >= i.total).collect();
    let owing: Vec<_> = invoices.iter().filter(|i| i.paid < i.total).collect();
    for (label, group) in [("Paid in full", &paid), ("Outstanding", &owing)] {
        section.row(vec![
            Cell::text(label),
            Cell::from(group.len() as i64),
            Cell::from(group.iter().map(|i| i.total).sum::<i64>()),
            Cell::from(group.iter().map(|i| i.paid).sum::<i64>()),
            Cell::from(group.iter().map(|i| i.total - i.paid).sum::<i64>()),
        ]);
    }
    section.totals = Some(vec![
        Cell::text("Total"),
        Cell::from(invoices.len() as i64),
        Cell::from(invoices.iter().map(|i| i.total).sum::<i64>()),
        Cell::from(invoices.iter().map(|i| i.paid).sum::<i64>()),
        Cell::from(invoices.iter().map(|i| i.total - i.paid).sum::<i64>()),
    ]);
    report.sections.push(section);
    report
        .notes
        .push("Income excludes GST. On the cash basis it counts payments received; on the accrual basis, invoices issued.".into());
    Ok(report)
}

async fn customer_name(conn: &mut PgConnection, id: Option<Uuid>) -> AppResult<Option<String>> {
    let Some(id) = id else { return Ok(None) };
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM customers WHERE id = $1")
        .bind(id)
        .fetch_optional(conn)
        .await?;
    name.map(Some)
        .ok_or_else(|| AppError::validation("customer_id", "Choose a customer."))
}

// ─── Expense summary ────────────────────────────────────────────────────────

async fn expense_summary(
    conn: &mut PgConnection,
    c: &Ctx,
    params: &ReportParams,
) -> AppResult<Report> {
    let mut report = c.report(ReportKind::ExpenseSummary, true, true);
    let account_id = match params.category_id {
        Some(id) => {
            let row: Option<(Uuid, String)> =
                sqlx::query_as("SELECT account_id, name FROM expense_categories WHERE id = $1")
                    .bind(id)
                    .fetch_optional(&mut *conn)
                    .await?;
            let (account, name) =
                row.ok_or_else(|| AppError::validation("category_id", "Choose a category."))?;
            report.subtitle.push(format!("Category: {name}"));
            Some(account)
        }
        None => None,
    };
    let lines = repo::expense_lines(conn, c.from, c.to, c.basis, account_id).await?;
    let business: Vec<_> = lines.iter().filter(|l| l.kind == "expense").collect();
    let total: i64 = business.iter().map(|l| l.amount).sum();
    let gst: i64 = lines
        .iter()
        .filter(|l| l.kind == "gst")
        .map(|l| l.amount)
        .sum();
    let private: i64 = lines
        .iter()
        .filter(|l| l.kind == "private")
        .map(|l| l.amount)
        .sum();
    let amount_columns = |first: &str| vec![Column::new(first, Text), Column::new("Amount", Money)];

    let mut by_category: BTreeMap<String, i64> = BTreeMap::new();
    for l in &business {
        *by_category.entry(l.account_name.clone()).or_default() += l.amount;
    }
    let mut categories: Vec<(String, i64)> = by_category.into_iter().collect();
    categories.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut section = Section::new("By category", amount_columns("Category"));
    for (name, amount) in categories {
        section.row(vec![Cell::text(name), Cell::from(amount)]);
    }
    section.totals = Some(vec![Cell::text("Total"), Cell::from(total)]);
    report.sections.push(section);

    let mut by_month: BTreeMap<NaiveDate, [i64; 3]> = months_between(c.from, c.to)
        .into_iter()
        .map(|m| (m, [0; 3]))
        .collect();
    for l in &lines {
        let slot = match l.kind.as_str() {
            "expense" => 0,
            "gst" => 1,
            _ => 2,
        };
        by_month.entry(l.month).or_default()[slot] += l.amount;
    }
    let mut section = Section::new(
        "By month",
        vec![
            Column::new("Month", Text),
            Column::new("Business expenses", Money),
            Column::new("GST claimed", Money),
            Column::new("Private share", Money),
        ],
    );
    for (month, [expense, gst, private]) in &by_month {
        section.row(vec![
            Cell::text(month_label(*month)),
            Cell::from(*expense),
            Cell::from(*gst),
            Cell::from(*private),
        ]);
    }
    section.totals = Some(vec![
        Cell::text("Total"),
        Cell::from(total),
        Cell::from(gst),
        Cell::from(private),
    ]);
    report.sections.push(section);

    let mut by_supplier: BTreeMap<String, i64> = BTreeMap::new();
    for l in &business {
        let name = if l.supplier.is_empty() {
            "No supplier".into()
        } else {
            l.supplier.clone()
        };
        *by_supplier.entry(name).or_default() += l.amount;
    }
    let mut suppliers: Vec<(String, i64)> = by_supplier.into_iter().collect();
    suppliers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut section = Section::new("By supplier", amount_columns("Supplier"));
    for (name, amount) in suppliers {
        section.row(vec![Cell::text(name), Cell::from(amount)]);
    }
    section.totals = Some(vec![Cell::text("Total"), Cell::from(total)]);
    report.sections.push(section);

    let mut section = Section::new("Business and personal", amount_columns("")).describe(
        "What was spent, split into the business part, the GST claimed and the private share.",
    );
    section.row(vec![
        Cell::text("Business expenses (excluding GST)"),
        Cell::from(total),
    ]);
    section.row(vec![Cell::text("GST claimed"), Cell::from(gst)]);
    section.row(vec![
        Cell::text("Private share (owner drawings)"),
        Cell::from(private),
    ]);
    section.totals = Some(vec![
        Cell::text("Total spent"),
        Cell::from(total + gst + private),
    ]);
    report.sections.push(section);
    report
        .notes
        .push("Mileage claims are listed under their own category and supplier. GST is claimed only while GST-registered.".into());
    Ok(report)
}

// ─── Statement of account ───────────────────────────────────────────────────

async fn statement(conn: &mut PgConnection, c: &Ctx, params: &ReportParams) -> AppResult<Report> {
    let customer_id = params.customer_id.ok_or_else(|| {
        AppError::validation("customer_id", "Choose the customer the statement is for.")
    })?;
    let name = customer_name(conn, Some(customer_id))
        .await?
        .unwrap_or_default();
    let mut report = c.report(ReportKind::StatementOfAccount, true, false);
    report.subtitle.push(format!("For {name}"));
    // The business's contact details, so the customer knows who to ask.
    let contact: Vec<String> = [c.business.email.clone(), c.business.phone.clone()]
        .into_iter()
        .flatten()
        .filter(|v| !v.trim().is_empty())
        .chain(
            serde_json::from_value::<crate::domain::business::Address>(c.business.address.clone())
                .map(|a| a.lines().join(", "))
                .ok()
                .filter(|l| !l.is_empty()),
        )
        .collect();
    if !contact.is_empty() {
        report.subtitle.push(format!(
            "From {}: {}",
            report.business_name,
            contact.join(" · ")
        ));
    }
    let invoices = repo::issued_invoices(conn, Some(customer_id), false).await?;
    let payments = repo::customer_payments(conn, customer_id).await?;

    // Running balance: invoices add, payments subtract; everything before the period is brought forward.
    enum Item<'a> {
        Invoice(&'a repo::InvoiceBalance),
        Payment(&'a repo::CustomerPayment),
    }
    let mut items: Vec<(NaiveDate, u8, Item)> = invoices
        .iter()
        .map(|i| (i.issue_date, 0, Item::Invoice(i)))
        .chain(
            payments
                .iter()
                .map(|p| (p.payment_date, 1, Item::Payment(p))),
        )
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut opening = 0_i64;
    let mut balance;
    let mut section = Section::new(
        "Transactions",
        vec![
            Column::new("Date", Date),
            Column::new("Details", Text),
            Column::new("Invoiced", Money),
            Column::new("Paid", Money),
            Column::new("Balance", Money),
        ],
    );
    for (date, _, item) in items.iter().filter(|(d, _, _)| *d < c.from) {
        let _ = date;
        match item {
            Item::Invoice(i) => opening += i.total,
            Item::Payment(p) => opening -= p.amount,
        }
    }
    balance = opening;
    section.row(vec![
        Cell::date(c.from),
        Cell::text("Balance brought forward"),
        Cell::empty(),
        Cell::empty(),
        Cell::from(opening),
    ]);
    let (mut invoiced, mut paid) = (0_i64, 0_i64);
    for (date, _, item) in items.iter().filter(|(d, _, _)| *d >= c.from && *d <= c.to) {
        match item {
            Item::Invoice(i) => {
                balance += i.total;
                invoiced += i.total;
                section.row(vec![
                    Cell::date(*date),
                    Cell::text(format!("Invoice {}", i.number.clone().unwrap_or_default())),
                    Cell::from(i.total),
                    Cell::empty(),
                    Cell::from(balance),
                ]);
            }
            Item::Payment(p) => {
                balance -= p.amount;
                paid += p.amount;
                let reference = if p.reference.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", p.reference)
                };
                section.row(vec![
                    Cell::date(*date),
                    Cell::text(format!(
                        "Payment for invoice {}{reference}",
                        p.invoice_number.clone().unwrap_or_default()
                    )),
                    Cell::empty(),
                    Cell::from(p.amount),
                    Cell::from(balance),
                ]);
            }
        }
    }
    section.totals = Some(vec![
        Cell::empty(),
        Cell::text("Closing balance"),
        Cell::from(invoiced),
        Cell::from(paid),
        Cell::from(balance),
    ]);

    let mut summary = Section::new(
        "Summary",
        vec![Column::new("", Text), Column::new("Amount", Money)],
    );
    summary.row(vec![
        Cell::text("Balance brought forward"),
        Cell::from(opening),
    ]);
    summary.row(vec![
        Cell::text("Invoiced in the period"),
        Cell::from(invoiced),
    ]);
    summary.row(vec![Cell::text("Paid in the period"), Cell::from(paid)]);
    summary.row(vec![
        Cell::text("Credit balance"),
        Cell::from((-balance).max(0)),
    ]);
    summary.totals = Some(vec![Cell::text("Amount owing"), Cell::from(balance.max(0))]);
    report.sections.push(summary);
    report.sections.push(section);

    let mut outstanding = Section::new(
        "Outstanding invoices",
        vec![
            Column::new("Invoice", Text),
            Column::new("Issued", Date),
            Column::new("Due", Date),
            Column::new("Total", Money),
            Column::new("Paid", Money),
            Column::new("Owing", Money),
        ],
    )
    .describe(format!("As at {}.", c.today.format("%-d %b %Y")));
    let owing: Vec<_> = invoices.iter().filter(|i| i.paid < i.total).collect();
    for i in &owing {
        outstanding.row(vec![
            Cell::text(i.number.clone().unwrap_or_default()),
            Cell::date(i.issue_date),
            Cell::date(i.due_date),
            Cell::from(i.total),
            Cell::from(i.paid),
            Cell::from(i.total - i.paid),
        ]);
    }
    outstanding.totals = Some(vec![
        Cell::text("Total owing"),
        Cell::empty(),
        Cell::empty(),
        Cell::from(owing.iter().map(|i| i.total).sum::<i64>()),
        Cell::from(owing.iter().map(|i| i.paid).sum::<i64>()),
        Cell::from(owing.iter().map(|i| i.total - i.paid).sum::<i64>()),
    ]);
    report.sections.push(outstanding);
    report.notes.push(format!(
        "Amounts include GST where charged. Please contact {} with any questions about this statement.",
        report.business_name
    ));
    Ok(report)
}

// ─── Accounts receivable ────────────────────────────────────────────────────

async fn receivables(conn: &mut PgConnection, c: &Ctx, params: &ReportParams) -> AppResult<Report> {
    let mut report = c.report(ReportKind::Receivables, false, false);
    report
        .subtitle
        .push(format!("As at {}", c.today.format("%-d %b %Y")));
    if let Some(name) = customer_name(conn, params.customer_id).await? {
        report.subtitle.push(format!("Customer: {name}"));
    }
    let owing = repo::issued_invoices(conn, params.customer_id, true).await?;
    let bucket_columns = || {
        let mut columns = vec![Column::new("Customer", Text)];
        columns.extend(Bucket::ALL.iter().map(|b| Column::new(b.label(), Money)));
        columns.push(Column::new("Total", Money));
        columns
    };
    let mut by_customer: BTreeMap<String, [i64; 5]> = BTreeMap::new();
    let mut totals = [0_i64; 5];
    for i in &owing {
        let bucket = ageing_bucket(i.due_date, c.today).index();
        by_customer.entry(i.customer_name.clone()).or_default()[bucket] += i.total - i.paid;
        totals[bucket] += i.total - i.paid;
    }
    let mut section = Section::new("Ageing by customer", bucket_columns())
        .describe("Unpaid amounts by days past the due date.");
    let mut customers: Vec<(String, [i64; 5])> = by_customer.into_iter().collect();
    customers.sort_by(|a, b| {
        b.1.iter()
            .sum::<i64>()
            .cmp(&a.1.iter().sum::<i64>())
            .then(a.0.cmp(&b.0))
    });
    for (name, buckets) in &customers {
        let mut row = vec![Cell::text(name.clone())];
        row.extend(buckets.iter().map(|v| Cell::from(*v)));
        row.push(Cell::from(buckets.iter().sum::<i64>()));
        section.row(row);
    }
    let mut total_row = vec![Cell::text("Total")];
    total_row.extend(totals.iter().map(|v| Cell::from(*v)));
    total_row.push(Cell::from(totals.iter().sum::<i64>()));
    section.totals = Some(total_row);
    report.sections.push(section);

    let mut list = Section::new(
        "Outstanding invoices",
        vec![
            Column::new("Invoice", Text),
            Column::new("Customer", Text),
            Column::new("Due", Date),
            Column::new("Days overdue", Count),
            Column::new("Owing", Money),
        ],
    );
    let mut sorted: Vec<_> = owing.iter().collect();
    sorted.sort_by_key(|i| (i.due_date, i.number.clone()));
    for i in sorted {
        list.row(vec![
            Cell::text(i.number.clone().unwrap_or_default()),
            Cell::text(i.customer_name.clone()),
            Cell::date(i.due_date),
            Cell::from((c.today - i.due_date).num_days().max(0)),
            Cell::from(i.total - i.paid),
        ]);
    }
    list.totals = Some(vec![
        Cell::text("Total"),
        Cell::empty(),
        Cell::empty(),
        Cell::empty(),
        Cell::from(owing.iter().map(|i| i.total - i.paid).sum::<i64>()),
    ]);
    report.sections.push(list);
    Ok(report)
}

// ─── Financial statement ────────────────────────────────────────────────────

async fn financial_statement(conn: &mut PgConnection, c: &Ctx) -> AppResult<Report> {
    let mut report = c.report(ReportKind::FinancialStatement, true, true);
    let months = repo::monthly(conn, c.from, c.to, c.basis).await?;
    let mut section = Section::new(
        "By month",
        vec![
            Column::new("Month", Text),
            Column::new("Income", Money),
            Column::new("Expenses", Money),
            Column::new("Net", Money),
            Column::new("Net to date", Money),
        ],
    );
    let mut running = 0;
    for m in &months {
        running += m.income - m.expenses;
        section.row(vec![
            Cell::text(month_label(m.month)),
            Cell::from(m.income),
            Cell::from(m.expenses),
            Cell::from(m.income - m.expenses),
            Cell::from(running),
        ]);
    }
    let profit = repo::profit(conn, c.from, c.to, c.basis).await?;
    section.totals = Some(vec![
        Cell::text("Total"),
        Cell::from(profit.income),
        Cell::from(profit.expenses),
        Cell::from(profit.net_profit),
        Cell::empty(),
    ]);
    let months_with_income = months.iter().filter(|m| m.income != 0).count() as i64;
    let mut summary = Section::new(
        "Summary",
        vec![Column::new("", Text), Column::new("Amount", Money)],
    );
    summary.row(vec![Cell::text("Income"), Cell::from(profit.income)]);
    summary.row(vec![Cell::text("Expenses"), Cell::from(profit.expenses)]);
    summary.row(vec![
        Cell::text("Net profit"),
        Cell::from(profit.net_profit),
    ]);
    if !months.is_empty() {
        summary.row(vec![
            Cell::text("Average monthly income"),
            Cell::from(profit.income / months.len() as i64),
        ]);
    }
    let receivables = repo::receivables(conn, c.today, c.from, c.to).await?;
    summary.row(vec![
        Cell::text("Owed by customers today"),
        Cell::from(receivables.outstanding_amount),
    ]);
    report.sections.push(summary);
    report.sections.push(section);
    report.subtitle.push(format!(
        "{} of {} months with income",
        months_with_income,
        months.len()
    ));
    report.notes.push(
        "Prepared from the business’s own records; not audited or reviewed by an accountant."
            .into(),
    );
    Ok(report)
}

// ─── GST ────────────────────────────────────────────────────────────────────

async fn gst(conn: &mut PgConnection, c: &Ctx) -> AppResult<Report> {
    let mut report = c.report(ReportKind::Gst, true, true);
    let entries = repo::gst_entries(conn, c.from, c.to, c.basis).await?;
    let collected: i64 = entries.iter().map(|e| e.collected).sum();
    let claimable: i64 = entries.iter().map(|e| e.claimable).sum();
    let mut summary = Section::new(
        "Summary",
        vec![Column::new("", Text), Column::new("Amount", Money)],
    );
    summary.row(vec![
        Cell::text("GST collected on sales and income"),
        Cell::from(collected),
    ]);
    summary.row(vec![
        Cell::text("GST claimable on expenses"),
        Cell::from(claimable),
    ]);
    summary.totals = Some(vec![
        Cell::text(if collected >= claimable {
            "GST to pay"
        } else {
            "GST refund"
        }),
        Cell::from((collected - claimable).abs()),
    ]);
    report.sections.push(summary);

    let mut by_month: BTreeMap<NaiveDate, [i64; 2]> = months_between(c.from, c.to)
        .into_iter()
        .map(|m| (m, [0; 2]))
        .collect();
    for e in &entries {
        let month = e.entry_date.with_day0(0).unwrap_or(e.entry_date);
        let slot = by_month.entry(month).or_default();
        slot[0] += e.collected;
        slot[1] += e.claimable;
    }
    let mut section = Section::new(
        "By month",
        vec![
            Column::new("Month", Text),
            Column::new("Collected", Money),
            Column::new("Claimable", Money),
            Column::new("Net", Money),
        ],
    );
    for (month, [col, claim]) in &by_month {
        section.row(vec![
            Cell::text(month_label(*month)),
            Cell::from(*col),
            Cell::from(*claim),
            Cell::from(col - claim),
        ]);
    }
    section.totals = Some(vec![
        Cell::text("Total"),
        Cell::from(collected),
        Cell::from(claimable),
        Cell::from(collected - claimable),
    ]);
    report.sections.push(section);

    let mut records = Section::new(
        "Records",
        vec![
            Column::new("Date", Date),
            Column::new("Record", Text),
            Column::new("Collected", Money),
            Column::new("Claimable", Money),
        ],
    )
    .describe("Every entry behind the figures above; corrections appear as their own entries.");
    for e in &entries {
        records.row(vec![
            Cell::date(e.entry_date),
            Cell::text(e.description.clone()),
            Cell::from(e.collected),
            Cell::from(e.claimable),
        ]);
    }
    records.totals = Some(vec![
        Cell::empty(),
        Cell::text("Total"),
        Cell::from(collected),
        Cell::from(claimable),
    ]);
    report.sections.push(records);
    if !c.business.gst_registered {
        report
            .notes
            .push("This business isn’t GST-registered, so no GST is recorded.".into());
    }
    report.notes.push(
        "Informational, from the business’s records: not a filed GST return. Check it with your accountant or IRD before filing.".into(),
    );
    Ok(report)
}

// ─── Allocations ────────────────────────────────────────────────────────────

fn rule_text(r: &alloc_repo::RuleRow, c: &Ctx) -> String {
    let amount = match (r.percentage, r.fixed_amount) {
        (Some(p), _) => format!(
            "{}% of {}",
            p.normalize(),
            if r.basis == "remaining" {
                "what remains"
            } else {
                "the amount received"
            }
        ),
        (None, Some(f)) => format!("{} from each receipt", c.money(f)),
        _ => String::new(),
    };
    let mut details = Vec::new();
    match (r.source.as_str(), r.income_type.as_deref()) {
        ("invoice_payment", _) => details.push("invoice payments only".to_owned()),
        ("income", Some("sales")) => details.push("sales income only".to_owned()),
        ("income", Some("other")) => details.push("other income only".to_owned()),
        ("income", _) => details.push("income only".to_owned()),
        _ => {}
    }
    if let Some(customer) = &r.customer_name {
        details.push(format!("from {customer}"));
    }
    if let Some(max) = r.max_amount {
        details.push(format!("at most {} per receipt", c.money(max)));
    }
    if let (Some(cap), Some(period)) = (r.cap_amount, r.cap_period.as_deref()) {
        let period = match period {
            "week" => "a week".to_owned(),
            "fortnight" => "a fortnight".to_owned(),
            "month" => "a month".to_owned(),
            _ => format!("every {} days", r.cap_period_days.unwrap_or(1)),
        };
        details.push(format!("at most {} {period}", c.money(cap)));
    }
    if r.stop_at_target {
        details.push("stops at the target".to_owned());
    }
    if details.is_empty() {
        amount
    } else {
        format!("{amount} ({})", details.join(", "))
    }
}

async fn allocations(conn: &mut PgConnection, c: &Ctx, params: &ReportParams) -> AppResult<Report> {
    let mut report = c.report(ReportKind::Allocations, true, false);
    let destinations = alloc_repo::list_destinations(conn).await?;
    let filter = alloc_repo::AllocationFilter {
        status: params.status.clone(),
        from: Some(c.from),
        to: Some(c.to),
        ..Default::default()
    };
    let history = alloc_repo::list_allocations(conn, &filter, 10_000, 0).await?;
    let ids: Vec<Uuid> = history.iter().map(|a| a.id).collect();
    let mut in_period: BTreeMap<Uuid, i64> = BTreeMap::new();
    for e in alloc_repo::entries(conn, &ids).await? {
        *in_period.entry(e.destination_id).or_default() += e.amount;
    }
    let spending = alloc_repo::list_spending(conn, None, Some(c.from), Some(c.to), false).await?;
    let mut spent: BTreeMap<Uuid, i64> = BTreeMap::new();
    for s in &spending {
        *spent.entry(s.destination_id).or_default() += s.amount;
    }

    let mut balances = Section::new(
        "Balances",
        vec![
            Column::new("Destination", Text),
            Column::new("Set aside in the period", Money),
            Column::new("Spent in the period", Money),
            Column::new("Balance today", Money),
            Column::new("Target", Money),
            Column::new("Progress", Number),
        ],
    );
    for d in destinations
        .iter()
        .filter(|d| !d.archived || d.balance != 0)
    {
        balances.row(vec![
            Cell::text(if d.archived {
                format!("{} (archived)", d.name)
            } else {
                d.name.clone()
            }),
            Cell::from(in_period.get(&d.id).copied().unwrap_or(0)),
            Cell::from(spent.get(&d.id).copied().unwrap_or(0)),
            Cell::from(d.balance),
            d.target_amount.map_or(Cell::empty(), Cell::from),
            d.target_amount.map_or(Cell::empty(), |t| {
                Cell::text(format!(
                    "{}%",
                    i128::from(d.balance.max(0)) * 100 / i128::from(t)
                ))
            }),
        ]);
    }
    balances.totals = Some(vec![
        Cell::text("Total"),
        Cell::from(in_period.values().sum::<i64>()),
        Cell::from(spent.values().sum::<i64>()),
        Cell::from(destinations.iter().map(|d| d.balance).sum::<i64>()),
        Cell::empty(),
        Cell::empty(),
    ]);
    report.sections.push(balances);

    if !spending.is_empty() {
        let mut section = Section::new(
            "Spending",
            vec![
                Column::new("Date", Date),
                Column::new("From", Text),
                Column::new("What for", Text),
                Column::new("Amount", Money),
            ],
        )
        .describe(
            "Money used from what was set aside (recorded only: nothing is paid by the app).",
        );
        for s in spending.iter().rev() {
            section.row(vec![
                Cell::date(s.spent_on),
                Cell::text(s.destination_name.clone()),
                Cell::text(s.description.clone()),
                Cell::from(s.amount),
            ]);
        }
        section.totals = Some(vec![
            Cell::empty(),
            Cell::empty(),
            Cell::text("Total"),
            Cell::from(spending.iter().map(|s| s.amount).sum::<i64>()),
        ]);
        report.sections.push(section);
    }

    let mut rules = Section::new(
        "Rules",
        vec![
            Column::new("Order", Count),
            Column::new("Destination", Text),
            Column::new("Sets aside", Text),
        ],
    )
    .describe("Applied in this order to what is left of each receipt.");
    for (i, r) in alloc_repo::list_rules(conn)
        .await?
        .iter()
        .filter(|r| !r.archived)
        .enumerate()
    {
        rules.row(vec![
            Cell::from(i as i64 + 1),
            Cell::text(r.destination_name.clone()),
            Cell::text(rule_text(r, c)),
        ]);
    }
    report.sections.push(rules);

    let mut section = Section::new(
        "History",
        vec![
            Column::new("Received", Date),
            Column::new("From", Text),
            Column::new("Amount received", Money),
            Column::new("Allocated", Money),
            Column::new("Status", Text),
        ],
    );
    for a in history.iter().rev() {
        let status = match a.status.as_str() {
            "pending" => "To review",
            "confirmed" => "Allocated",
            "skipped" => "Skipped",
            "reversed" => "Reversed",
            _ => "Cancelled",
        };
        section.row(vec![
            Cell::date(a.received_date),
            Cell::text(a.source_label.clone()),
            Cell::from(a.received_amount),
            Cell::from(a.allocated),
            Cell::text(status),
        ]);
    }
    report.sections.push(section);
    report
        .notes
        .push("A plan for the money received: nothing here was moved or paid.".into());
    Ok(report)
}

// ─── Mileage ────────────────────────────────────────────────────────────────

async fn mileage_report(
    state: &AppState,
    ctx: &BusinessContext,
    c: Ctx,
    params: &ReportParams,
) -> AppResult<Report> {
    let summary = mileage::summary(state, ctx, Some(c.from), Some(c.to), params.vehicle_id).await?;
    let mut report = c.report(ReportKind::Mileage, true, false);
    let km = |d: rust_decimal::Decimal| Cell::text(d.to_string());
    let columns = |first: &str| {
        vec![
            Column::new(first, Text),
            Column::new("Trips", Count),
            Column::new("Distance (km)", Number),
            Column::new("Business (km)", Number),
            Column::new("Claim", Money),
        ]
    };
    let totals = |t: &crate::repositories::mileage::TripTotals| {
        vec![
            Cell::text("Total"),
            Cell::from(t.trips),
            km(t.distance_km),
            km(t.business_distance_km),
            Cell::from(t.claim_amount),
        ]
    };
    let mut by_month = Section::new("By month", columns("Month"));
    for m in &summary.monthly {
        by_month.row(vec![
            Cell::text(month_label(m.month)),
            Cell::from(m.totals.trips),
            km(m.totals.distance_km),
            km(m.totals.business_distance_km),
            Cell::from(m.totals.claim_amount),
        ]);
    }
    by_month.totals = Some(totals(&summary.totals));
    let mut by_vehicle = Section::new("By vehicle", columns("Vehicle"));
    for v in &summary.vehicles {
        by_vehicle.row(vec![
            Cell::text(v.vehicle_name.clone()),
            Cell::from(v.totals.trips),
            km(v.totals.distance_km),
            km(v.totals.business_distance_km),
            Cell::from(v.totals.claim_amount),
        ]);
    }
    by_vehicle.totals = Some(totals(&summary.totals));
    report.sections.push(by_month);
    report.sections.push(by_vehicle);
    report
        .notes
        .push("Claims use each trip’s own rate, and are included in expenses.".into());
    Ok(report)
}
