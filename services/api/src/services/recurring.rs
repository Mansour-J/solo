//! Recurring invoices (SL-701): a schedule made from an invoice repeats its lines. `run` creates the
//! invoices that have come due (drafts, or issued when the schedule says so) and moves each schedule to
//! its next date; the client calls it when the app opens.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::documents::LineInput;
use crate::domain::recurring::{Frequency, next};
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::recurring::{self as repo, RecurringFields, RecurringRow};
use crate::services::businesses::business_today;
use crate::services::invoices::{self, InvoiceInput, clean, load_business};
use crate::state::AppState;

/// Most invoices one schedule creates in one run (a long-forgotten weekly schedule catches up slowly).
const MAX_PER_RUN: usize = 24;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RecurringFromInvoice {
    /// The invoice to repeat: its customer, lines, terms and notes.
    pub invoice_id: Uuid,
    pub name: String,
    pub frequency: Frequency,
    /// The date of the first invoice the schedule creates.
    pub next_date: NaiveDate,
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
    #[serde(default)]
    pub auto_issue: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RecurringInput {
    pub name: String,
    pub frequency: Frequency,
    pub next_date: NaiveDate,
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
    #[serde(default)]
    pub auto_issue: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RecurringArchiveInput {
    pub archived: bool,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct RunResult {
    /// Invoices created.
    pub created: i64,
    /// Of those, issued straight away.
    pub issued: i64,
    /// Schedules that couldn't create their invoice, with why.
    pub problems: Vec<String>,
}

fn check(
    name: &str,
    next_date: NaiveDate,
    end_date: Option<NaiveDate>,
    errors: &mut FieldErrors,
) -> String {
    let name = clean(name, 120, "name", errors);
    errors.check(!name.is_empty(), "name", "Give the schedule a name.");
    if let Some(end) = end_date {
        errors.check(
            end >= next_date,
            "end_date",
            "The end date can’t be before the next invoice.",
        );
    }
    name
}

async fn record(
    conn: &mut sqlx::PgConnection,
    ctx: &BusinessContext,
    action: &str,
    id: Uuid,
    summary: String,
    changes: serde_json::Value,
) -> AppResult<()> {
    audit::record(
        conn,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action,
            entity_type: "recurring_invoice",
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await
    .map_err(Into::into)
}

pub async fn list(state: &AppState, ctx: &BusinessContext) -> AppResult<Vec<RecurringRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::list(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

/// Makes a schedule from an invoice (draft or issued): it repeats the invoice's customer, lines,
/// prices, reference, notes and payment terms.
pub async fn create(
    state: &AppState,
    ctx: &BusinessContext,
    input: RecurringFromInvoice,
) -> AppResult<RecurringRow> {
    let mut errors = FieldErrors::new();
    let name = check(&input.name, input.next_date, input.end_date, &mut errors);
    errors.into_result()?;
    let invoice = invoices::get(state, ctx, input.invoice_id).await?;
    if invoice.status == "cancelled" {
        return Err(AppError::Conflict(
            "A cancelled invoice can’t be repeated.".into(),
        ));
    }
    let lines: Vec<LineInput> = invoice
        .lines
        .iter()
        .map(|l| LineInput {
            description: l.description.clone(),
            quantity: l.quantity,
            unit_price: l.unit_price,
            discount_percent: l.discount_percent,
            tax_treatment: l.tax_treatment,
        })
        .collect();
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let id = repo::insert(
        &mut tx,
        ctx.business_id,
        &RecurringFields {
            customer_id: invoice.customer.id,
            name: name.clone(),
            frequency: input.frequency.as_str().into(),
            anchor_day: input.next_date.day() as i16,
            next_date: input.next_date,
            end_date: input.end_date,
            payment_terms_days: Some((invoice.due_date - invoice.issue_date).num_days() as i32),
            prices_include_gst: invoice.prices_include_gst,
            reference: invoice.reference.clone(),
            notes: invoice.notes.clone(),
            footer: invoice.footer.clone(),
            lines,
            auto_issue: input.auto_issue,
        },
    )
    .await?;
    record(
        &mut tx,
        ctx,
        "recurring_invoice.created",
        id,
        format!("Recurring invoice “{name}” set up"),
        serde_json::json!({ "invoice_id": input.invoice_id, "frequency": input.frequency.as_str(), "next_date": input.next_date }),
    )
    .await?;
    let row = repo::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Recurring invoice"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: RecurringInput,
) -> AppResult<RecurringRow> {
    let mut errors = FieldErrors::new();
    let name = check(&input.name, input.next_date, input.end_date, &mut errors);
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = repo::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Recurring invoice"))?;
    repo::update(
        &mut tx,
        id,
        &repo::ScheduleChange {
            name: &name,
            frequency: input.frequency.as_str(),
            anchor_day: input.next_date.day() as i16,
            next_date: input.next_date,
            end_date: input.end_date,
            auto_issue: input.auto_issue,
        },
    )
    .await?;
    record(
        &mut tx,
        ctx,
        "recurring_invoice.updated",
        id,
        format!("Recurring invoice “{name}” changed"),
        serde_json::json!({
            "frequency": { "from": before.frequency, "to": input.frequency.as_str() },
            "next_date": { "from": before.next_date, "to": input.next_date },
            "end_date": { "from": before.end_date, "to": input.end_date },
            "auto_issue": { "from": before.auto_issue, "to": input.auto_issue },
        }),
    )
    .await?;
    let row = repo::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Recurring invoice"))?;
    tx.commit().await?;
    Ok(row)
}

/// Stops (archives) a schedule, or starts it again.
pub async fn archive(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    archived: bool,
) -> AppResult<RecurringRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Recurring invoice"))?;
    if !archived && row.end_date.is_some_and(|end| end < row.next_date) {
        return Err(AppError::Conflict(
            "This schedule has ended: change its end date first.".into(),
        ));
    }
    repo::set_archived(&mut tx, id, archived).await?;
    record(
        &mut tx,
        ctx,
        if archived {
            "recurring_invoice.stopped"
        } else {
            "recurring_invoice.restarted"
        },
        id,
        format!(
            "Recurring invoice “{}” {}",
            row.name,
            if archived { "stopped" } else { "restarted" }
        ),
        serde_json::json!({}),
    )
    .await?;
    let row = repo::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Recurring invoice"))?;
    tx.commit().await?;
    Ok(row)
}

/// Why an invoice couldn't be created or issued, for the list of problems.
fn problem(error: &AppError) -> String {
    match error {
        AppError::Validation(errors) => errors.0.values().cloned().collect::<Vec<_>>().join(" "),
        other => other.public_message(),
    }
}

/// Creates every invoice that has come due, oldest first, and moves each schedule on.
pub async fn run(state: &AppState, ctx: &BusinessContext) -> AppResult<RunResult> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let schedules = repo::due(&mut tx, today).await?;
    tx.commit().await?;
    let mut result = RunResult::default();
    for schedule in schedules {
        let Some(frequency) = Frequency::parse(&schedule.frequency) else {
            continue;
        };
        let mut date = schedule.next_date;
        for _ in 0..MAX_PER_RUN {
            if date > today || schedule.end_date.is_some_and(|end| date > end) {
                break;
            }
            let input = InvoiceInput {
                customer_id: schedule.customer_id,
                issue_date: date,
                due_date: schedule
                    .payment_terms_days
                    .map(|days| date + chrono::Duration::days(i64::from(days))),
                reference: schedule.reference.clone(),
                prices_include_gst: schedule.prices_include_gst,
                notes: Some(schedule.notes.clone()),
                footer: Some(schedule.footer.clone()),
                lines: schedule.lines.0.clone(),
            };
            let invoice = match invoices::create(state, ctx, input).await {
                Ok(invoice) => invoice,
                Err(error) => {
                    result.problems.push(format!(
                        "{}: couldn’t create the invoice for {} ({})",
                        schedule.name,
                        date.format("%d/%m/%Y"),
                        problem(&error)
                    ));
                    break;
                }
            };
            result.created += 1;
            if schedule.auto_issue {
                match invoices::issue(state, ctx, invoice.id).await {
                    Ok(_) => result.issued += 1,
                    Err(error) => result.problems.push(format!(
                        "{}: the invoice for {} was created as a draft but couldn’t be issued ({})",
                        schedule.name,
                        date.format("%d/%m/%Y"),
                        problem(&error)
                    )),
                }
            }
            let following = next(date, frequency, schedule.anchor_day as u32);
            let finished = schedule.end_date.is_some_and(|end| following > end);
            let mut tx = state
                .db
                .begin_business(ctx.user_id(), ctx.business_id)
                .await?;
            repo::advance(&mut tx, schedule.id, following, invoice.id, finished).await?;
            record(
                &mut tx,
                ctx,
                "recurring_invoice.invoice_created",
                schedule.id,
                format!(
                    "Recurring invoice “{}”: invoice for {} created",
                    schedule.name,
                    date.format("%d/%m/%Y")
                ),
                serde_json::json!({ "invoice_id": invoice.id, "issue_date": date }),
            )
            .await?;
            tx.commit().await?;
            date = following;
            if finished {
                break;
            }
        }
    }
    Ok(result)
}
