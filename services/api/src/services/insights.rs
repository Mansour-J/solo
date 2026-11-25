//! Insights (ADR 0012): what the figures mean, in plain language. Every insight is a deterministic
//! rule over the same journal queries the reports use (`repositories::reports`) plus a few record
//! queries (`repositories::insights`), with explicit thresholds in `domain::insights`. An insight
//! without enough data behind it isn't shown; estimates are marked as estimates.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::context::BusinessContext;
use crate::domain::business::{ReportingBasis, financial_year};
use crate::domain::insights::{
    CONCENTRATION_WARNING_PERCENT, Figure, MIN_NOTABLE_CHANGE, MIN_SAMPLE, Tone, Trend, average,
    must_register_for_gst, notable_growth, percent_change, percent_of, recurring, trend,
    unusual_spend,
};
use crate::domain::money::Currency;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::allocations as alloc_repo;
use crate::repositories::insights as repo;
use crate::repositories::reports::{self, MonthTotals};
use crate::services::businesses::business_today;
use crate::services::invoices::{clean, load_business};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Detail {
    pub label: String,
    pub figure: Figure,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Insight {
    pub key: String,
    pub title: String,
    /// The headline figure, if the insight has one.
    pub figure: Option<Figure>,
    /// What it means, in plain language.
    pub statement: String,
    /// How it was worked out (and from what).
    pub explanation: String,
    pub tone: Tone,
    /// A forecast or estimate rather than recorded figures.
    pub estimate: bool,
    /// Where to see the records behind it (an app route).
    pub link: Option<String>,
    pub details: Vec<Detail>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InsightSection {
    /// `performance`, `health`, `spending` or `cash_flow`.
    pub key: String,
    pub title: String,
    pub description: String,
    pub items: Vec<Insight>,
    /// Why there's nothing to show yet, when `items` is empty.
    pub empty_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Notice {
    /// Changes when the facts behind the notice change, so a dismissed notice comes back then.
    pub key: String,
    pub message: String,
    pub tone: Tone,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Insights {
    pub generated_on: NaiveDate,
    pub currency: String,
    /// `cash` or `accrual` (the business's reporting basis).
    pub basis: String,
    pub sections: Vec<InsightSection>,
    /// Notices this user hasn't dismissed.
    pub notices: Vec<Notice>,
    /// How many notices this user has dismissed in this business.
    pub dismissed_count: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DismissInput {
    pub key: String,
}

/// Dates the rules look at, from the business's today.
struct Window {
    today: NaiveDate,
    this_month: NaiveDate,
    last_month: NaiveDate,
    month_before: NaiveDate,
    /// First day of the twelve complete months before this one.
    start12: NaiveDate,
    fy_start: NaiveDate,
}

impl Window {
    fn new(today: NaiveDate, fy_start: NaiveDate) -> Self {
        let this_month = today.with_day(1).unwrap_or(today);
        let back = |n: u32| {
            this_month
                .checked_sub_months(Months::new(n))
                .unwrap_or(this_month)
        };
        Self {
            today,
            this_month,
            last_month: back(1),
            month_before: back(2),
            start12: back(12),
            fy_start,
        }
    }

    fn end_of_last_month(&self) -> NaiveDate {
        self.this_month - Duration::days(1)
    }
}

fn month_name(month: NaiveDate) -> String {
    month.format("%B").to_string()
}

fn month_label(month: NaiveDate) -> String {
    month.format("%b %Y").to_string()
}

struct Writer {
    currency: Currency,
}

impl Writer {
    fn money(&self, minor: i64) -> String {
        self.currency.format(minor)
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn higher_lower(change: i64) -> String {
    if change >= 0 {
        format!("{change}% higher")
    } else {
        format!("{}% lower", -change)
    }
}

/// Every input the rules need, read in one transaction.
struct Data {
    months: Vec<MonthTotals>,
    fy: reports::ProfitTotals,
    fy_last_year: reports::ProfitTotals,
    income: Vec<reports::IncomeLine>,
    expenses: Vec<reports::ExpenseLine>,
    invoices: Vec<reports::InvoiceBalance>,
    payment_days: Vec<i64>,
    outstanding_history: Vec<(NaiveDate, i64)>,
    money_in_month: i64,
    money_in_fy: i64,
    money_out_fy: i64,
    set_aside: i64,
    allocations_pending: i64,
    gst_registered: bool,
    /// Income over the last 12 months (for the GST registration threshold).
    sales_last_12_months: i64,
}

async fn load(
    conn: &mut PgConnection,
    w: &Window,
    basis: &str,
    gst_registered: bool,
) -> AppResult<Data> {
    let end_last = w.end_of_last_month();
    let months = reports::monthly(conn, w.start12, end_last, basis).await?;
    let fy = reports::profit(conn, w.fy_start, w.today, basis).await?;
    let year_ago = |d: NaiveDate| d.checked_sub_months(Months::new(12)).unwrap_or(d);
    let fy_last_year =
        reports::profit(conn, year_ago(w.fy_start), year_ago(w.today), basis).await?;
    let income = reports::income_lines(conn, w.start12, end_last, basis, None).await?;
    let expenses = reports::expense_lines(conn, w.start12, w.today, basis, None).await?;
    let invoices = reports::issued_invoices(conn, None, false).await?;
    let payment_days = repo::payment_times(conn, w.start12, w.today)
        .await?
        .into_iter()
        .map(|(issued, paid)| (paid - issued).num_days().max(0))
        .collect();
    let mut outstanding_history = Vec::new();
    for n in (1..=6).rev() {
        let month_end = w
            .this_month
            .checked_sub_months(Months::new(n - 1))
            .unwrap_or(w.this_month)
            - Duration::days(1);
        outstanding_history.push((month_end, repo::outstanding_at(conn, month_end).await?));
    }
    let (money_in_month, _) = repo::money_in_out(conn, w.this_month, w.today).await?;
    let (money_in_fy, money_out_fy) = repo::money_in_out(conn, w.fy_start, w.today).await?;
    let set_aside = alloc_repo::balances(conn)
        .await?
        .iter()
        .map(|(_, b)| b)
        .sum();
    let allocations_pending = alloc_repo::pending_count(conn).await?;
    let year_ago = w
        .today
        .checked_sub_months(Months::new(12))
        .unwrap_or(w.today)
        + Duration::days(1);
    let sales_last_12_months = reports::profit(conn, year_ago, w.today, basis)
        .await?
        .income;
    Ok(Data {
        months,
        fy,
        fy_last_year,
        income,
        expenses,
        invoices,
        payment_days,
        outstanding_history,
        money_in_month,
        money_in_fy,
        money_out_fy,
        set_aside,
        allocations_pending,
        gst_registered,
        sales_last_12_months,
    })
}

fn insight(
    key: &str,
    title: &str,
    figure: Option<Figure>,
    statement: String,
    explanation: &str,
    tone: Tone,
) -> Insight {
    Insight {
        key: key.into(),
        title: title.into(),
        figure,
        statement,
        explanation: explanation.into(),
        tone,
        estimate: false,
        link: None,
        details: Vec::new(),
    }
}

// ─── Financial performance ──────────────────────────────────────────────────

fn performance(d: &Data, w: &Window, x: &Writer) -> Vec<Insight> {
    let mut items = Vec::new();
    let last = d.months.last();
    let before = d.months.len().checked_sub(2).and_then(|i| d.months.get(i));
    if let (Some(last), Some(before)) = (last, before) {
        if let Some(change) = percent_change(last.income, before.income) {
            items.push(insight(
                "revenue-month",
                "Revenue, month on month",
                Some(Figure::percent(change)),
                format!(
                    "Revenue in {} was {} than in {} ({} against {}).",
                    month_name(w.last_month),
                    higher_lower(change),
                    month_name(w.month_before),
                    x.money(last.income),
                    x.money(before.income)
                ),
                "The last complete month compared with the one before, excluding GST.",
                if change >= 0 {
                    Tone::Positive
                } else {
                    Tone::Negative
                },
            ));
        }
        if let Some(change) = percent_change(last.expenses, before.expenses) {
            items.push(insight(
                "expenses-month",
                "Expenses, month on month",
                Some(Figure::percent(change)),
                format!(
                    "Expenses in {} were {} than in {} ({} against {}).",
                    month_name(w.last_month),
                    higher_lower(change),
                    month_name(w.month_before),
                    x.money(last.expenses),
                    x.money(before.expenses)
                ),
                "The last complete month compared with the one before, excluding GST and private use.",
                if change > 0 { Tone::Warning } else { Tone::Positive },
            ));
        }
    }
    if let Some(change) = percent_change(d.fy.income, d.fy_last_year.income) {
        items.push(insight(
            "revenue-year",
            "Revenue this financial year",
            Some(Figure::percent(change)),
            format!(
                "Revenue so far this financial year is {} than at the same point last year ({} against {}).",
                higher_lower(change),
                x.money(d.fy.income),
                x.money(d.fy_last_year.income)
            ),
            "From the start of the financial year to today, against the same dates a year earlier.",
            if change >= 0 { Tone::Positive } else { Tone::Negative },
        ));
    }
    let sum = |range: std::ops::Range<usize>, f: fn(&MonthTotals) -> i64| -> i64 {
        d.months.get(range).map_or(0, |m| m.iter().map(f).sum())
    };
    if d.months.len() == 12 {
        for (key, title, f, noun, rising_tone) in [
            (
                "revenue-growth",
                "Revenue growth",
                (|m: &MonthTotals| m.income) as fn(&MonthTotals) -> i64,
                "Revenue",
                Tone::Positive,
            ),
            (
                "expense-growth",
                "Expense growth",
                |m: &MonthTotals| m.expenses,
                "Expenses",
                Tone::Warning,
            ),
        ] {
            let recent = sum(9..12, f);
            let previous = sum(6..9, f);
            if let Some(change) = percent_change(recent, previous) {
                items.push(insight(
                    key,
                    title,
                    Some(Figure::percent(change)),
                    format!(
                        "{noun} over the last three months was {} than in the three months before ({} against {}).",
                        higher_lower(change),
                        x.money(recent),
                        x.money(previous)
                    ),
                    "The last three complete months against the three before them.",
                    if change >= 0 { rising_tone } else if rising_tone == Tone::Positive { Tone::Negative } else { Tone::Positive },
                ));
            }
        }
        let net: Vec<i64> = d.months.iter().map(|m| m.income - m.expenses).collect();
        let active = d.months[6..]
            .iter()
            .filter(|m| m.income != 0 || m.expenses != 0)
            .count();
        if active >= 4
            && let Some(direction) = trend(&net, 3)
        {
            let mut item = insight(
                "profit-trend",
                "Profit trend",
                Some(Figure::money(net[9..].iter().sum())),
                match direction {
                    Trend::Rising => "Profit is rising: the last three months made more than the three before.".into(),
                    Trend::Falling => "Profit is falling: the last three months made less than the three before.".into(),
                    Trend::Steady => "Profit is steady: the last three months made about the same as the three before.".into(),
                },
                "Revenue less expenses for each of the last six complete months; the figure is the last three months together.",
                match direction {
                    Trend::Rising => Tone::Positive,
                    Trend::Falling => Tone::Negative,
                    Trend::Steady => Tone::Neutral,
                },
            );
            item.details = d.months[6..]
                .iter()
                .map(|m| Detail {
                    label: month_label(m.month),
                    figure: Figure::money(m.income - m.expenses),
                })
                .collect();
            items.push(item);
        }
        // Averages over the months since activity began in the window.
        if let Some(first) = d
            .months
            .iter()
            .position(|m| m.income != 0 || m.expenses != 0)
        {
            let span = &d.months[first..];
            if span.len() >= 3 {
                let incomes: Vec<i64> = span.iter().map(|m| m.income).collect();
                let costs: Vec<i64> = span.iter().map(|m| m.expenses).collect();
                let (avg_income, avg_costs) =
                    (average(&incomes).unwrap_or(0), average(&costs).unwrap_or(0));
                let mut item = insight(
                    "monthly-averages",
                    "Average month",
                    Some(Figure::money(avg_income)),
                    format!(
                        "An average month brings in {} and costs {}, leaving {}.",
                        x.money(avg_income),
                        x.money(avg_costs),
                        x.money(avg_income - avg_costs)
                    ),
                    &format!(
                        "Averaged over the last {} complete months with records.",
                        span.len()
                    ),
                    Tone::Neutral,
                );
                item.details = vec![
                    Detail {
                        label: "Average monthly revenue".into(),
                        figure: Figure::money(avg_income),
                    },
                    Detail {
                        label: "Average monthly expenses".into(),
                        figure: Figure::money(avg_costs),
                    },
                    Detail {
                        label: "Months counted".into(),
                        figure: Figure::count(span.len() as i64),
                    },
                ];
                items.push(item);
            }
        }
    }
    items
}

// ─── Business health ────────────────────────────────────────────────────────

fn health(d: &Data, w: &Window, x: &Writer) -> Vec<Insight> {
    let mut items = Vec::new();
    // Revenue by customer over the last twelve complete months.
    let mut by_customer: BTreeMap<String, i64> = BTreeMap::new();
    for l in d.income.iter().filter(|l| l.customer_id.is_some()) {
        *by_customer
            .entry(l.customer_name.clone().unwrap_or_default())
            .or_default() += l.amount;
    }
    let mut customers: Vec<(String, i64)> =
        by_customer.into_iter().filter(|(_, v)| *v > 0).collect();
    customers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let from_customers: i64 = customers.iter().map(|c| c.1).sum();
    if customers.len() >= 2
        && let Some(top_share) = percent_of(customers[0].1, from_customers)
    {
        let mut item = insight(
            "concentration",
            "Revenue concentration",
            Some(Figure::percent(top_share)),
            format!(
                "{} brought in {top_share}% of your revenue from customers over the last 12 months.{}",
                customers[0].0,
                if top_share >= CONCENTRATION_WARNING_PERCENT {
                    " Relying on one customer this much is a risk if they stop buying."
                } else {
                    ""
                }
            ),
            "Revenue from each customer over the last 12 complete months, excluding GST; income with no customer isn't counted.",
            if top_share >= CONCENTRATION_WARNING_PERCENT {
                Tone::Warning
            } else {
                Tone::Neutral
            },
        );
        item.link = Some("/reports?report=income_summary".into());
        item.details = customers
            .iter()
            .take(5)
            .map(|(name, amount)| Detail {
                label: name.clone(),
                figure: Figure::money(*amount),
            })
            .collect();
        items.push(item);
    }

    // Customers with overdue balances.
    let overdue: Vec<&reports::InvoiceBalance> = d
        .invoices
        .iter()
        .filter(|i| i.paid < i.total && i.due_date < w.today)
        .collect();
    if !overdue.is_empty() {
        let mut owing: BTreeMap<String, i64> = BTreeMap::new();
        for i in &overdue {
            *owing.entry(i.customer_name.clone()).or_default() += i.total - i.paid;
        }
        let mut owing: Vec<(String, i64)> = owing.into_iter().collect();
        owing.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let total: i64 = owing.iter().map(|o| o.1).sum();
        let mut item = insight(
            "overdue-customers",
            "Overdue balances",
            Some(Figure::money(total)),
            format!(
                "{} with overdue invoices owe {} in total.",
                plural(owing.len() as i64, "customer", "customers"),
                x.money(total)
            ),
            "Issued invoices past their due date with an unpaid balance, as at today.",
            Tone::Warning,
        );
        item.link = Some("/reports?report=receivables".into());
        item.details = owing
            .into_iter()
            .map(|(name, amount)| Detail {
                label: name,
                figure: Figure::money(amount),
            })
            .collect();
        items.push(item);
    }

    // Average invoice value.
    let recent: Vec<i64> = d
        .invoices
        .iter()
        .filter(|i| i.issue_date >= w.start12)
        .map(|i| i.total)
        .collect();
    if recent.len() >= MIN_SAMPLE
        && let Some(avg) = average(&recent)
    {
        items.push(insight(
            "invoice-value",
            "Average invoice",
            Some(Figure::money(avg)),
            format!(
                "Your invoices average {} ({} issued in the last 12 months).",
                x.money(avg),
                plural(recent.len() as i64, "invoice", "invoices")
            ),
            "The total of each invoice issued in the last 12 months, including GST, averaged.",
            Tone::Neutral,
        ));
    }

    // Average payment time.
    if d.payment_days.len() >= MIN_SAMPLE
        && let Some(days) = average(&d.payment_days)
    {
        items.push(insight(
            "payment-time",
            "Average time to be paid",
            Some(Figure::days(days)),
            format!(
                "Customers take {} on average to pay an invoice in full.",
                plural(days, "day", "days")
            ),
            "Days from the invoice date to its last payment, for invoices paid in full in the last 12 months.",
            if days > 30 { Tone::Warning } else { Tone::Neutral },
        ));
    }

    // Unpaid invoices over time.
    if d.outstanding_history.iter().any(|(_, v)| *v > 0) {
        let now: i64 = d.invoices.iter().map(|i| (i.total - i.paid).max(0)).sum();
        let (then_date, then) =
            d.outstanding_history[d.outstanding_history.len().saturating_sub(3)];
        let statement = if then == 0 {
            format!(
                "Customers owe {} now; nothing was owed at the end of {}.",
                x.money(now),
                month_label(then_date)
            )
        } else {
            format!(
                "Customers owe {} now, against {} at the end of {}.",
                x.money(now),
                x.money(then),
                month_label(then_date)
            )
        };
        let mut item = insight(
            "unpaid-trend",
            "Unpaid invoices",
            Some(Figure::money(now)),
            statement,
            "What customers owed at the end of each of the last six months, from invoices and payments recorded.",
            if now > then + MIN_NOTABLE_CHANGE {
                Tone::Warning
            } else if now < then {
                Tone::Positive
            } else {
                Tone::Neutral
            },
        );
        item.link = Some("/reports?report=receivables".into());
        item.details = d
            .outstanding_history
            .iter()
            .map(|(date, amount)| Detail {
                label: month_label(*date),
                figure: Figure::money(*amount),
            })
            .collect();
        items.push(item);
    }

    // Expense-to-revenue ratio.
    let revenue: i64 = d.months.iter().map(|m| m.income).sum();
    let costs: i64 = d.months.iter().map(|m| m.expenses).sum();
    if let Some(ratio) = percent_of(costs, revenue)
        && costs > 0
    {
        items.push(insight(
            "expense-ratio",
            "Expenses against revenue",
            Some(Figure::percent(ratio)),
            format!(
                "Expenses were {ratio}% of revenue over the last 12 months ({} against {}).",
                x.money(costs),
                x.money(revenue)
            ),
            "Expenses divided by revenue over the last 12 complete months, both excluding GST.",
            if ratio >= 100 {
                Tone::Negative
            } else if ratio >= 70 {
                Tone::Warning
            } else {
                Tone::Positive
            },
        ));
    }
    items
}

// ─── Spending ───────────────────────────────────────────────────────────────

/// Business expenses per category, per month of the window (index 0 = 12 months ago, 11 = last
/// month), and month to date.
struct Spending {
    categories: BTreeMap<String, ([i64; 12], i64)>,
    suppliers: BTreeMap<String, [i64; 12]>,
}

fn month_index(w: &Window, month: NaiveDate) -> Option<usize> {
    let diff =
        (month.year() - w.start12.year()) * 12 + month.month() as i32 - w.start12.month() as i32;
    (0..12).contains(&diff).then_some(diff as usize)
}

fn spending_data(d: &Data, w: &Window) -> Spending {
    let mut categories: BTreeMap<String, ([i64; 12], i64)> = BTreeMap::new();
    let mut suppliers: BTreeMap<String, [i64; 12]> = BTreeMap::new();
    for l in d.expenses.iter().filter(|l| l.kind == "expense") {
        let entry = categories.entry(l.account_name.clone()).or_default();
        if l.month >= w.this_month {
            entry.1 += l.amount;
        } else if let Some(i) = month_index(w, l.month) {
            entry.0[i] += l.amount;
            if !l.supplier.is_empty() && l.supplier != "Mileage claims" {
                suppliers.entry(l.supplier.clone()).or_default()[i] += l.amount;
            }
        }
    }
    Spending {
        categories,
        suppliers,
    }
}

fn spending(s: &Spending, d: &Data, x: &Writer) -> Vec<Insight> {
    let mut items = Vec::new();
    let last3 = |m: &[i64; 12]| m[9..].iter().sum::<i64>();
    let prev3 = |m: &[i64; 12]| m[6..9].iter().sum::<i64>();

    let mut biggest: Vec<(&String, i64)> = s
        .categories
        .iter()
        .map(|(k, v)| (k, last3(&v.0)))
        .filter(|(_, v)| *v > 0)
        .collect();
    biggest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let total: i64 = biggest.iter().map(|b| b.1).sum();
    if let Some((name, amount)) = biggest.first() {
        let share = percent_of(*amount, total).unwrap_or(0);
        let mut item = insight(
            "biggest-categories",
            "Biggest expenses",
            Some(Figure::money(*amount)),
            format!(
                "{name} was your biggest expense over the last three months: {} ({share}% of the total).",
                x.money(*amount)
            ),
            "Business expenses by category over the last three complete months, excluding GST.",
            Tone::Neutral,
        );
        item.link = Some("/reports?report=expense_summary".into());
        item.details = biggest
            .iter()
            .take(3)
            .map(|(name, amount)| Detail {
                label: (*name).clone(),
                figure: Figure::money(*amount),
            })
            .collect();
        items.push(item);
    }

    let mut growing: Vec<(&String, i64, i64)> = s
        .categories
        .iter()
        .filter_map(|(k, v)| notable_growth(last3(&v.0), prev3(&v.0)).map(|c| (k, c, last3(&v.0))))
        .collect();
    growing.sort_by_key(|g| std::cmp::Reverse(g.1));
    if let Some((name, change, _)) = growing.first() {
        let mut item = insight(
            "growing-categories",
            "Growing expenses",
            Some(Figure::percent(*change)),
            format!(
                "{name} spending rose {change}% over the last three months compared with the three before.{}",
                if growing.len() > 1 {
                    format!(" {} more categories also grew.", growing.len() - 1)
                } else {
                    String::new()
                }
            ),
            "Categories up at least 20% and at least $50 over the last three complete months against the three before.",
            Tone::Warning,
        );
        item.details = growing
            .iter()
            .map(|(name, change, _)| Detail {
                label: (*name).clone(),
                figure: Figure::percent(*change),
            })
            .collect();
        items.push(item);
    }

    let mut unusual: Vec<(&String, i64, i64)> = s
        .categories
        .iter()
        .filter_map(|(k, v)| unusual_spend(v.0[11], &v.0[5..11]).map(|usual| (k, v.0[11], usual)))
        .collect();
    unusual.sort_by_key(|u| std::cmp::Reverse(u.1 - u.2));
    if let Some((name, spent, usual)) = unusual.first() {
        let mut item = insight(
            "unusual-spending",
            "Unusual spending last month",
            Some(Figure::money(*spent)),
            format!(
                "{name} cost {} last month, well above the usual {} a month.",
                x.money(*spent),
                x.money(*usual)
            ),
            "Last month against the average of the previous six months with spending (at least three), flagged at 1.5 times usual and $100 more.",
            Tone::Warning,
        );
        item.details = unusual
            .iter()
            .map(|(name, spent, _)| Detail {
                label: (*name).clone(),
                figure: Figure::money(*spent),
            })
            .collect();
        items.push(item);
    }

    // Monthly pattern: busiest and quietest months for spending.
    let with_spend: Vec<&MonthTotals> = d.months.iter().filter(|m| m.expenses > 0).collect();
    if with_spend.len() >= 6 {
        let high = with_spend.iter().max_by_key(|m| m.expenses).copied();
        let low = with_spend.iter().min_by_key(|m| m.expenses).copied();
        let avg = average(&with_spend.iter().map(|m| m.expenses).collect::<Vec<_>>()).unwrap_or(0);
        if let (Some(high), Some(low)) = (high, low) {
            let mut item = insight(
                "spending-pattern",
                "Monthly spending",
                Some(Figure::money(avg)),
                format!(
                    "You usually spend about {} a month; most in {} ({}) and least in {} ({}).",
                    x.money(avg),
                    month_label(high.month),
                    x.money(high.expenses),
                    month_label(low.month),
                    x.money(low.expenses)
                ),
                "Business expenses per month over the last 12 complete months with spending, excluding GST.",
                Tone::Neutral,
            );
            item.details = d
                .months
                .iter()
                .map(|m| Detail {
                    label: month_label(m.month),
                    figure: Figure::money(m.expenses),
                })
                .collect();
            items.push(item);
        }
    }

    let mut commitments: Vec<(&String, i64)> = s
        .suppliers
        .iter()
        .filter_map(|(k, v)| recurring(&v[9..]).map(|typical| (k, typical)))
        .collect();
    commitments.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    if !commitments.is_empty() {
        let total: i64 = commitments.iter().map(|c| c.1).sum();
        let mut item = insight(
            "recurring",
            "Regular commitments",
            Some(Figure::money(total)),
            format!(
                "{} you pay about the same every month, around {} a month together.",
                plural(commitments.len() as i64, "supplier", "suppliers"),
                x.money(total)
            ),
            "Suppliers paid in each of the last three complete months, every month within 10% of their average.",
            Tone::Neutral,
        );
        item.details = commitments
            .iter()
            .map(|(name, typical)| Detail {
                label: (*name).clone(),
                figure: Figure::money(*typical),
            })
            .collect();
        items.push(item);
    }
    items
}

// ─── Cash flow ──────────────────────────────────────────────────────────────

fn cash_flow(d: &Data, w: &Window, x: &Writer) -> Vec<Insight> {
    let mut items = Vec::new();
    let owing: Vec<&reports::InvoiceBalance> =
        d.invoices.iter().filter(|i| i.paid < i.total).collect();
    let outstanding: i64 = owing.iter().map(|i| i.total - i.paid).sum();
    let overdue: i64 = owing
        .iter()
        .filter(|i| i.due_date < w.today)
        .map(|i| i.total - i.paid)
        .sum();
    if outstanding > 0 {
        let mut item = insight(
            "receivables",
            "Owed to you",
            Some(Figure::money(outstanding)),
            format!(
                "Customers owe you {} on {}{}.",
                x.money(outstanding),
                plural(owing.len() as i64, "invoice", "invoices"),
                if overdue > 0 {
                    format!(", {} of it overdue", x.money(overdue))
                } else {
                    String::new()
                }
            ),
            "Recorded: unpaid balances of issued invoices, as at today.",
            if overdue > 0 {
                Tone::Warning
            } else {
                Tone::Neutral
            },
        );
        item.link = Some("/reports?report=receivables".into());
        items.push(item);

        let horizon = w.today + Duration::days(30);
        let due_soon: Vec<&&reports::InvoiceBalance> = owing
            .iter()
            .filter(|i| i.due_date >= w.today && i.due_date <= horizon)
            .collect();
        let expected: i64 = due_soon.iter().map(|i| i.total - i.paid).sum();
        if expected > 0 {
            let mut item = insight(
                "expected-payments",
                "Expected in the next 30 days",
                Some(Figure::money(expected)),
                format!(
                    "If customers pay on time, {} could arrive in the next 30 days from {}.",
                    x.money(expected),
                    plural(due_soon.len() as i64, "invoice", "invoices")
                ),
                "An estimate: unpaid balances of invoices due in the next 30 days. Overdue invoices aren't counted, since when they'll be paid is unknown.",
                Tone::Neutral,
            );
            item.estimate = true;
            items.push(item);
        }
    }
    if d.money_in_month > 0 {
        items.push(insight(
            "received-this-month",
            "Received this month",
            Some(Figure::money(d.money_in_month)),
            format!(
                "You've recorded {} received so far this month.",
                x.money(d.money_in_month)
            ),
            "Recorded: invoice payments and income dated this month, including GST.",
            Tone::Neutral,
        ));
    }
    if d.set_aside != 0 || d.allocations_pending > 0 {
        let mut item = insight(
            "set-aside",
            "Set aside",
            Some(Figure::money(d.set_aside)),
            format!(
                "You've planned to set aside {} across your allocations{}.",
                x.money(d.set_aside),
                if d.allocations_pending > 0 {
                    format!(
                        ", with {} still to review",
                        plural(d.allocations_pending, "payment", "payments")
                    )
                } else {
                    String::new()
                }
            ),
            "Planned: confirmed allocation balances. A plan for the money, not money moved.",
            Tone::Neutral,
        );
        item.link = Some("/allocations".into());
        items.push(item);
    }
    if d.money_in_fy > 0 {
        let available = d.money_in_fy - d.money_out_fy - d.set_aside;
        let mut item = insight(
            "available-funds",
            "Estimated available funds",
            Some(Figure::money(available)),
            format!(
                "About {} of what you've received this financial year isn't spent or set aside.",
                x.money(available)
            ),
            "An estimate: money received this financial year (payments and income, including GST) less expenses paid from business funds, less amounts set aside. It doesn't know your bank balance, money from before this year, or drawings.",
            if available < 0 {
                Tone::Warning
            } else {
                Tone::Neutral
            },
        );
        item.estimate = true;
        item.details = vec![
            Detail {
                label: "Received this financial year".into(),
                figure: Figure::money(d.money_in_fy),
            },
            Detail {
                label: "Spent from business funds".into(),
                figure: Figure::money(d.money_out_fy),
            },
            Detail {
                label: "Set aside".into(),
                figure: Figure::money(d.set_aside),
            },
        ];
        items.push(item);
    }
    items
}

// ─── Notices ────────────────────────────────────────────────────────────────

fn notices(d: &Data, s: &Spending, w: &Window, x: &Writer) -> Vec<Notice> {
    let mut out = Vec::new();
    if must_register_for_gst(d.gst_registered, d.sales_last_12_months) {
        out.push(Notice {
            key: "gst-threshold".into(),
            message: format!(
                "Your sales over the last 12 months were {}, over the $60,000 GST threshold. IRD requires you to register for GST within 21 days.",
                x.money(d.sales_last_12_months)
            ),
            tone: Tone::Warning,
            link: Some("/settings/business".into()),
        });
    }
    let overdue: Vec<&reports::InvoiceBalance> = d
        .invoices
        .iter()
        .filter(|i| i.paid < i.total && i.due_date < w.today)
        .collect();
    if !overdue.is_empty() {
        let total: i64 = overdue.iter().map(|i| i.total - i.paid).sum();
        out.push(Notice {
            key: format!("overdue:{}:{total}", overdue.len()),
            message: format!(
                "You have {} ({}).",
                plural(overdue.len() as i64, "overdue invoice", "overdue invoices"),
                x.money(total)
            ),
            tone: Tone::Warning,
            link: Some("/reports?report=receivables".into()),
        });
    }
    if d.allocations_pending > 0 {
        out.push(Notice {
            key: format!("allocations:{}", d.allocations_pending),
            message: format!(
                "{} waiting for you to confirm how to set {} aside.",
                plural(d.allocations_pending, "payment is", "payments are"),
                if d.allocations_pending == 1 {
                    "it"
                } else {
                    "them"
                }
            ),
            tone: Tone::Neutral,
            link: Some("/allocations".into()),
        });
    }
    let last = d.months.last();
    let before = d.months.len().checked_sub(2).and_then(|i| d.months.get(i));
    if let (Some(last), Some(before)) = (last, before)
        && let Some(change) = percent_change(last.income, before.income)
        && change != 0
    {
        out.push(Notice {
            key: format!("revenue-month:{}", w.last_month),
            message: format!(
                "Your revenue in {} was {} than in {}.",
                month_name(w.last_month),
                higher_lower(change),
                month_name(w.month_before)
            ),
            tone: if change > 0 {
                Tone::Positive
            } else {
                Tone::Neutral
            },
            link: None,
        });
    }
    // The category that rose most last month (at least 15% and $50).
    let rise = s
        .categories
        .iter()
        .filter_map(|(name, (m, _))| {
            let change = percent_change(m[11], m[10])?;
            (change >= 15 && m[11] - m[10] >= MIN_NOTABLE_CHANGE).then_some((name, change))
        })
        .max_by_key(|(_, c)| *c);
    if let Some((name, change)) = rise {
        out.push(Notice {
            key: format!("category-up:{}:{name}", w.last_month),
            message: format!(
                "Spending on {} in {} was {change}% higher than in {}.",
                name,
                month_name(w.last_month),
                month_name(w.month_before)
            ),
            tone: Tone::Warning,
            link: Some("/reports?report=expense_summary".into()),
        });
    }
    let this_month: Vec<(&String, i64)> = s
        .categories
        .iter()
        .map(|(k, v)| (k, v.1))
        .filter(|(_, v)| *v > 0)
        .collect();
    if this_month.len() >= 2
        && let Some((name, amount)) = this_month.iter().max_by_key(|(_, v)| *v)
    {
        out.push(Notice {
            key: format!("top-category:{}:{name}", w.this_month),
            message: format!(
                "{} is your highest-spending category this month ({}).",
                name,
                x.money(*amount)
            ),
            tone: Tone::Neutral,
            link: None,
        });
    }
    let spent = |i: usize| s.categories.values().map(|v| v.0[i]).sum::<i64>();
    let this_month_total: i64 = s.categories.values().map(|v| v.1).sum();
    if w.today.day() >= 15 && spent(11) > 0 && spent(10) > 0 && this_month_total == 0 {
        out.push(Notice {
            key: format!("no-expenses:{}", w.this_month),
            message: "You haven’t recorded an expense this month.".into(),
            tone: Tone::Neutral,
            link: Some("/expenses?new=1".into()),
        });
    }
    out
}

// ─── Endpoints ──────────────────────────────────────────────────────────────

pub async fn build(state: &AppState, ctx: &BusinessContext) -> AppResult<Insights> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let (fy_start, _) = financial_year(
        today,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let w = Window::new(today, fy_start);
    let basis = match business.reporting_basis {
        ReportingBasis::Cash => "cash",
        ReportingBasis::Accrual => "accrual",
    };
    let currency = Currency::from_code(&business.currency).ok_or_else(|| {
        AppError::internal(anyhow::anyhow!("unknown currency {}", business.currency))
    })?;
    let x = Writer { currency };
    let data = load(&mut tx, &w, basis, business.gst_registered).await?;
    let dismissed = repo::dismissed(&mut tx, ctx.user_id()).await?;
    tx.commit().await?;

    let spend = spending_data(&data, &w);
    let section = |key: &str, title: &str, description: &str, items: Vec<Insight>, empty: &str| {
        InsightSection {
            key: key.into(),
            title: title.into(),
            description: description.into(),
            empty_reason: items.is_empty().then(|| empty.to_owned()),
            items,
        }
    };
    let sections = vec![
        section(
            "performance",
            "Financial performance",
            "How revenue, expenses and profit are moving.",
            performance(&data, &w, &x),
            "These appear once there are at least two complete months of income and expenses to compare.",
        ),
        section(
            "health",
            "Business health",
            "Who you rely on, how quickly you’re paid and what’s owed.",
            health(&data, &w, &x),
            "These appear once you’ve issued a few invoices and been paid for them.",
        ),
        section(
            "spending",
            "Spending",
            "Where the money goes, and what’s changing.",
            spending(&spend, &data, &x),
            "These appear once you’ve recorded expenses over a few months.",
        ),
        section(
            "cash_flow",
            "Cash flow",
            "What’s been received, what’s owed and what’s set aside. Estimates are marked.",
            cash_flow(&data, &w, &x),
            "This appears once you’ve recorded money received or invoices to be paid.",
        ),
    ];
    let all_notices = notices(&data, &spend, &w, &x);
    Ok(Insights {
        generated_on: today,
        currency: business.currency.clone(),
        basis: basis.into(),
        sections,
        notices: all_notices
            .into_iter()
            .filter(|n| !dismissed.contains(&n.key))
            .collect(),
        dismissed_count: dismissed.len() as i64,
    })
}

/// Hides a notice for this user until the facts behind it change. Personal, so any member can.
pub async fn dismiss(
    state: &AppState,
    ctx: &BusinessContext,
    input: DismissInput,
) -> AppResult<()> {
    let mut errors = FieldErrors::new();
    let key = clean(&input.key, 200, "key", &mut errors);
    errors.check(!key.is_empty(), "key", "Name the notice to dismiss.");
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    repo::dismiss(&mut tx, ctx.business_id, ctx.user_id(), &key).await?;
    tx.commit().await?;
    Ok(())
}

/// Shows every notice this user dismissed in this business again.
pub async fn restore(state: &AppState, ctx: &BusinessContext) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    repo::restore_all(&mut tx, ctx.user_id()).await?;
    tx.commit().await?;
    Ok(())
}
