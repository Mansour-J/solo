//! Allocations: planning where money received should go (ADR 0010). Bookkeeping only: Solo never
//! moves money.
//!
//! Each receipt (an invoice payment or an income record) gets a pending allocation when the
//! business has rules. The user reviews the plan, confirms it (possibly adjusted) or skips it.
//! Confirmed amounts are append-only entries; a correction adds reversing entries and, while the
//! receipt still exists, a new pending allocation for it. Balances are the sum of entries.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::allocations::{
    self as calc, Basis, Cap, CapPeriod, Limit, PlanContext, Receipt, ReceiptSource, Rule,
    RuleAmount, SourceCondition, period_containing,
};
use crate::domain::money::Currency;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::allocations::{
    self as repo, AllocationFilter, AllocationRow, DestinationFields, DestinationRow, EntryRow,
    RuleFields, RuleRow, SourceRef,
};
use crate::repositories::audit;
use crate::services::income::IncomeType;
use crate::services::invoices::{clean, load_business};
use crate::state::AppState;

async fn audit_record(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    action: &str,
    entity_type: &str,
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
            entity_type,
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await
    .map_err(Into::into)
}

async fn currency(conn: &mut PgConnection, ctx: &BusinessContext) -> AppResult<Currency> {
    let business = load_business(conn, ctx.business_id).await?;
    Currency::from_code(&business.currency).ok_or_else(|| {
        AppError::internal(anyhow::anyhow!(
            "unknown business currency {}",
            business.currency
        ))
    })
}

pub use crate::services::mileage::ArchiveInput;

/// Ids in the order they should appear (destinations) or apply (rules).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct OrderInput {
    pub ids: Vec<Uuid>,
}

// ─── Destinations ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DestinationKind {
    IncomeTax,
    Gst,
    Acc,
    Savings,
    EmergencyFund,
    Kiwisaver,
    Investment,
    Mortgage,
    PersonalSpending,
    Custom,
}

impl DestinationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::IncomeTax => "income_tax",
            Self::Gst => "gst",
            Self::Acc => "acc",
            Self::Savings => "savings",
            Self::EmergencyFund => "emergency_fund",
            Self::Kiwisaver => "kiwisaver",
            Self::Investment => "investment",
            Self::Mortgage => "mortgage",
            Self::PersonalSpending => "personal_spending",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DestinationInput {
    pub name: String,
    #[serde(default = "custom_kind")]
    pub kind: DestinationKind,
    /// Minor units.
    #[serde(default)]
    pub target_amount: Option<i64>,
    /// Minor units.
    #[serde(default)]
    pub minimum_balance: Option<i64>,
    #[serde(default)]
    pub notes: String,
}

fn custom_kind() -> DestinationKind {
    DestinationKind::Custom
}

/// Largest amount accepted anywhere in allocations (a billion in major units).
const MAX_AMOUNT: i64 = 100_000_000_000;

fn check_amount(amount: Option<i64>, field: &str, errors: &mut FieldErrors) {
    if let Some(amount) = amount {
        errors.check(
            amount > 0 && amount <= MAX_AMOUNT,
            field,
            "Enter an amount greater than zero.",
        );
    }
}

async fn destination_fields(
    conn: &mut PgConnection,
    input: &DestinationInput,
    except: Option<Uuid>,
) -> AppResult<DestinationFields> {
    let mut errors = FieldErrors::new();
    let name = clean(&input.name, 80, "name", &mut errors);
    errors.check(!name.is_empty(), "name", "Enter a name.");
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    check_amount(input.target_amount, "target_amount", &mut errors);
    check_amount(input.minimum_balance, "minimum_balance", &mut errors);
    errors.into_result()?;
    if repo::destination_name_taken(conn, &name, except).await? {
        return Err(AppError::validation(
            "name",
            "A destination with this name already exists.",
        ));
    }
    Ok(DestinationFields {
        name,
        kind: input.kind.as_str().into(),
        target_amount: input.target_amount,
        minimum_balance: input.minimum_balance,
        notes,
    })
}

async fn find_destination(conn: &mut PgConnection, id: Uuid) -> AppResult<DestinationRow> {
    repo::find_destination(conn, id)
        .await?
        .ok_or(AppError::NotFound("Destination"))
}

pub async fn list_destinations(
    state: &AppState,
    ctx: &BusinessContext,
) -> AppResult<Vec<DestinationRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::list_destinations(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

pub async fn create_destination(
    state: &AppState,
    ctx: &BusinessContext,
    input: DestinationInput,
) -> AppResult<DestinationRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let fields = destination_fields(&mut tx, &input, None).await?;
    let id = repo::insert_destination(&mut tx, ctx.business_id, &fields).await?;
    audit_record(
        &mut tx,
        ctx,
        "allocation_destination.created",
        "allocation_destination",
        id,
        format!("Allocation destination “{}” added", fields.name),
        serde_json::json!({ "target_amount": fields.target_amount, "minimum_balance": fields.minimum_balance }),
    )
    .await?;
    let row = find_destination(&mut tx, id).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update_destination(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: DestinationInput,
) -> AppResult<DestinationRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = find_destination(&mut tx, id).await?;
    if before.archived {
        return Err(AppError::Conflict(
            "Restore this destination before changing it.".into(),
        ));
    }
    let fields = destination_fields(&mut tx, &input, Some(id)).await?;
    repo::update_destination(&mut tx, id, &fields).await?;
    audit_record(
        &mut tx,
        ctx,
        "allocation_destination.updated",
        "allocation_destination",
        id,
        format!("Allocation destination “{}” updated", fields.name),
        serde_json::json!({
            "name": { "from": before.name, "to": fields.name },
            "target_amount": { "from": before.target_amount, "to": fields.target_amount },
            "minimum_balance": { "from": before.minimum_balance, "to": fields.minimum_balance },
        }),
    )
    .await?;
    let row = find_destination(&mut tx, id).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn archive_destination(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    archived: bool,
) -> AppResult<DestinationRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = find_destination(&mut tx, id).await?;
    if archived && row.rule_count > 0 {
        return Err(AppError::Conflict(
            "Rules still allocate to this destination. Archive them or move them to another destination first.".into(),
        ));
    }
    if !archived
        && row.archived
        && repo::destination_name_taken(&mut tx, &row.name, Some(id)).await?
    {
        return Err(AppError::Conflict(
            "Another destination has this name. Rename one of them first.".into(),
        ));
    }
    repo::set_destination_archived(&mut tx, id, archived).await?;
    audit_record(
        &mut tx,
        ctx,
        if archived {
            "allocation_destination.archived"
        } else {
            "allocation_destination.restored"
        },
        "allocation_destination",
        id,
        format!(
            "Allocation destination “{}” {}",
            row.name,
            if archived { "archived" } else { "restored" }
        ),
        serde_json::json!({}),
    )
    .await?;
    let row = find_destination(&mut tx, id).await?;
    tx.commit().await?;
    Ok(row)
}

/// Orders the active destinations as given (every active destination exactly once).
pub async fn order_destinations(
    state: &AppState,
    ctx: &BusinessContext,
    input: OrderInput,
) -> AppResult<Vec<DestinationRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let active: Vec<Uuid> = repo::list_destinations(&mut tx)
        .await?
        .into_iter()
        .filter(|d| !d.archived)
        .map(|d| d.id)
        .collect();
    check_order(&input.ids, &active)?;
    for (position, id) in input.ids.iter().enumerate() {
        repo::set_destination_position(&mut tx, *id, position as i32 + 1).await?;
    }
    audit_record(
        &mut tx,
        ctx,
        "allocation_destination.reordered",
        "allocation_destination",
        input.ids[0],
        "Allocation destinations reordered".into(),
        serde_json::json!({ "ids": input.ids }),
    )
    .await?;
    let rows = repo::list_destinations(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

fn check_order(ids: &[Uuid], active: &[Uuid]) -> AppResult<()> {
    let mut given = ids.to_vec();
    given.sort();
    let mut expected = active.to_vec();
    expected.sort();
    if given.is_empty() || given != expected {
        return Err(AppError::validation(
            "ids",
            "List every active item exactly once.",
        ));
    }
    Ok(())
}

// ─── Rules ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleMethod {
    Percentage,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleBasis {
    /// Of the amount received.
    Received,
    /// Of what earlier rules left.
    Remaining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleSource {
    Any,
    InvoicePayment,
    Income,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapPeriodKind {
    Week,
    Fortnight,
    Month,
    Custom,
}

impl CapPeriodKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Fortnight => "fortnight",
            Self::Month => "month",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RuleInput {
    pub destination_id: Uuid,
    pub method: RuleMethod,
    /// For `percentage`: e.g. "20" or "12.5".
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub percentage: Option<Decimal>,
    #[serde(default = "default_basis")]
    pub basis: RuleBasis,
    /// For `fixed`: minor units.
    #[serde(default)]
    pub fixed_amount: Option<i64>,
    #[serde(default = "default_source")]
    pub source: RuleSource,
    /// Only with `source = income`: only sales or only other income.
    #[serde(default)]
    pub income_type: Option<IncomeType>,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    #[serde(default)]
    pub active_from: Option<NaiveDate>,
    #[serde(default)]
    pub active_to: Option<NaiveDate>,
    /// Most allocated from one receipt (minor units).
    #[serde(default)]
    pub max_amount: Option<i64>,
    /// Most allocated within each cap period (minor units).
    #[serde(default)]
    pub cap_amount: Option<i64>,
    #[serde(default)]
    pub cap_period: Option<CapPeriodKind>,
    /// Length of a `custom` cap period.
    #[serde(default)]
    pub cap_period_days: Option<i32>,
    /// First day of the first `fortnight` or `custom` period.
    #[serde(default)]
    pub cap_anchor: Option<NaiveDate>,
    #[serde(default)]
    pub stop_at_target: bool,
    #[serde(default)]
    pub notes: String,
}

fn default_basis() -> RuleBasis {
    RuleBasis::Received
}

fn default_source() -> RuleSource {
    RuleSource::Any
}

async fn rule_fields(
    conn: &mut PgConnection,
    input: &RuleInput,
    before: Option<&RuleRow>,
) -> AppResult<RuleFields> {
    let mut errors = FieldErrors::new();
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    match repo::find_destination(conn, input.destination_id).await? {
        None => errors.add("destination_id", "Choose a destination."),
        Some(d) if d.archived && before.is_none_or(|r| r.destination_id != d.id) => errors.add(
            "destination_id",
            "This destination is archived. Restore it or choose another.",
        ),
        Some(_) => {}
    }
    let (percentage, fixed_amount) = match input.method {
        RuleMethod::Percentage => {
            let p = input.percentage.map(|p| p.round_dp(2));
            errors.check(
                p.is_some_and(|p| p > Decimal::ZERO && p <= Decimal::ONE_HUNDRED),
                "percentage",
                "Enter a percentage greater than 0 and up to 100.",
            );
            (p, None)
        }
        RuleMethod::Fixed => {
            errors.check(
                input.fixed_amount.is_some(),
                "fixed_amount",
                "Enter an amount.",
            );
            check_amount(input.fixed_amount, "fixed_amount", &mut errors);
            (None, input.fixed_amount)
        }
    };
    let income_type = match (input.source, input.income_type) {
        (RuleSource::Income, Some(t)) => Some(t),
        (_, Some(_)) => {
            errors.add("income_type", "Only income can be limited to a type.");
            None
        }
        (_, None) => None,
    };
    if let Some(customer) = input.customer_id
        && !repo::customer_exists(conn, customer).await?
    {
        errors.add("customer_id", "Choose a customer.");
    }
    if let (Some(from), Some(to)) = (input.active_from, input.active_to) {
        errors.check(
            to >= from,
            "active_to",
            "The end date can’t be before the start date.",
        );
    }
    check_amount(input.max_amount, "max_amount", &mut errors);
    check_amount(input.cap_amount, "cap_amount", &mut errors);
    match (input.cap_amount, input.cap_period) {
        (Some(_), None) => errors.add("cap_period", "Choose the period the cap applies to."),
        (None, Some(_)) => errors.add("cap_amount", "Enter the cap for each period."),
        _ => {}
    }
    let period = input.cap_amount.and(input.cap_period);
    let needs_anchor = matches!(
        period,
        Some(CapPeriodKind::Fortnight | CapPeriodKind::Custom)
    );
    errors.check(
        !needs_anchor || input.cap_anchor.is_some(),
        "cap_anchor",
        "Enter the date the first period starts.",
    );
    let custom = period == Some(CapPeriodKind::Custom);
    if custom {
        errors.check(
            input
                .cap_period_days
                .is_some_and(|d| (1..=366).contains(&d)),
            "cap_period_days",
            "Enter a period of 1 to 366 days.",
        );
    }
    errors.into_result()?;
    Ok(RuleFields {
        destination_id: input.destination_id,
        method: match input.method {
            RuleMethod::Percentage => "percentage",
            RuleMethod::Fixed => "fixed",
        }
        .into(),
        percentage,
        basis: match input.basis {
            RuleBasis::Received => "received",
            RuleBasis::Remaining => "remaining",
        }
        .into(),
        fixed_amount,
        source: match input.source {
            RuleSource::Any => "any",
            RuleSource::InvoicePayment => "invoice_payment",
            RuleSource::Income => "income",
        }
        .into(),
        income_type: income_type.map(|t| {
            match t {
                IncomeType::Sales => "sales",
                IncomeType::Other => "other",
            }
            .into()
        }),
        customer_id: input.customer_id,
        active_from: input.active_from,
        active_to: input.active_to,
        max_amount: input.max_amount,
        cap_amount: period.and(input.cap_amount),
        cap_period: period.map(|p| p.as_str().into()),
        cap_period_days: if custom { input.cap_period_days } else { None },
        cap_anchor: if needs_anchor { input.cap_anchor } else { None },
        stop_at_target: input.stop_at_target,
        notes,
    })
}

async fn find_rule(conn: &mut PgConnection, id: Uuid) -> AppResult<RuleRow> {
    repo::find_rule(conn, id)
        .await?
        .ok_or(AppError::NotFound("Rule"))
}

fn rule_summary(row: &RuleRow) -> String {
    match (row.percentage, row.fixed_amount) {
        (Some(p), _) => format!("{}% to {}", p.normalize(), row.destination_name),
        (None, Some(_)) => format!("Fixed amount to {}", row.destination_name),
        _ => row.destination_name.clone(),
    }
}

pub async fn list_rules(state: &AppState, ctx: &BusinessContext) -> AppResult<Vec<RuleRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::list_rules(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

pub async fn create_rule(
    state: &AppState,
    ctx: &BusinessContext,
    input: RuleInput,
) -> AppResult<RuleRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let fields = rule_fields(&mut tx, &input, None).await?;
    let id = repo::insert_rule(&mut tx, ctx.business_id, &fields).await?;
    let row = find_rule(&mut tx, id).await?;
    audit_record(
        &mut tx,
        ctx,
        "allocation_rule.created",
        "allocation_rule",
        id,
        format!("Allocation rule added: {}", rule_summary(&row)),
        serde_json::json!({ "method": fields.method, "percentage": fields.percentage, "fixed_amount": fields.fixed_amount }),
    )
    .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update_rule(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: RuleInput,
) -> AppResult<RuleRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = find_rule(&mut tx, id).await?;
    if before.archived {
        return Err(AppError::Conflict(
            "Restore this rule before changing it.".into(),
        ));
    }
    let fields = rule_fields(&mut tx, &input, Some(&before)).await?;
    repo::update_rule(&mut tx, id, &fields).await?;
    let row = find_rule(&mut tx, id).await?;
    audit_record(
        &mut tx,
        ctx,
        "allocation_rule.updated",
        "allocation_rule",
        id,
        format!("Allocation rule updated: {}", rule_summary(&row)),
        serde_json::json!({
            "destination_id": { "from": before.destination_id, "to": fields.destination_id },
            "percentage": { "from": before.percentage, "to": fields.percentage },
            "fixed_amount": { "from": before.fixed_amount, "to": fields.fixed_amount },
            "cap_amount": { "from": before.cap_amount, "to": fields.cap_amount },
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn archive_rule(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    archived: bool,
) -> AppResult<RuleRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = find_rule(&mut tx, id).await?;
    if !archived
        && row.archived
        && repo::find_destination(&mut tx, row.destination_id)
            .await?
            .is_some_and(|d| d.archived)
    {
        return Err(AppError::Conflict(
            "Its destination is archived. Restore the destination first.".into(),
        ));
    }
    repo::set_rule_archived(&mut tx, id, archived).await?;
    audit_record(
        &mut tx,
        ctx,
        if archived {
            "allocation_rule.archived"
        } else {
            "allocation_rule.restored"
        },
        "allocation_rule",
        id,
        format!(
            "Allocation rule {}: {}",
            if archived { "archived" } else { "restored" },
            rule_summary(&row)
        ),
        serde_json::json!({}),
    )
    .await?;
    let row = find_rule(&mut tx, id).await?;
    tx.commit().await?;
    Ok(row)
}

/// Sets the priority of the active rules to the order given (every active rule exactly once).
pub async fn order_rules(
    state: &AppState,
    ctx: &BusinessContext,
    input: OrderInput,
) -> AppResult<Vec<RuleRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let active: Vec<Uuid> = repo::list_rules(&mut tx)
        .await?
        .into_iter()
        .filter(|r| !r.archived)
        .map(|r| r.id)
        .collect();
    check_order(&input.ids, &active)?;
    for (priority, id) in input.ids.iter().enumerate() {
        repo::set_rule_priority(&mut tx, *id, priority as i32 + 1).await?;
    }
    audit_record(
        &mut tx,
        ctx,
        "allocation_rule.reordered",
        "allocation_rule",
        input.ids[0],
        "Allocation rules reordered".into(),
        serde_json::json!({ "ids": input.ids }),
    )
    .await?;
    let rows = repo::list_rules(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

// ─── Plans ──────────────────────────────────────────────────────────────────

fn domain_rule(row: &RuleRow) -> Option<Rule> {
    let amount = match (row.method.as_str(), row.percentage, row.fixed_amount) {
        ("percentage", Some(percentage), _) => RuleAmount::Percentage {
            percentage,
            basis: if row.basis == "remaining" {
                Basis::Remaining
            } else {
                Basis::Received
            },
        },
        ("fixed", _, Some(amount)) => RuleAmount::Fixed(amount),
        _ => return None,
    };
    let source = match row.source.as_str() {
        "invoice_payment" => SourceCondition::InvoicePayments,
        "income" => SourceCondition::Income {
            sales: row.income_type.as_deref().map(|t| t == "sales"),
        },
        _ => SourceCondition::Any,
    };
    let cap = match (row.cap_amount, row.cap_period.as_deref()) {
        (Some(amount), Some(period)) => {
            let period = match (period, row.cap_anchor) {
                ("week", _) => CapPeriod::Week,
                ("month", _) => CapPeriod::Month,
                ("fortnight", Some(anchor)) => CapPeriod::Fortnight { anchor },
                ("custom", Some(anchor)) => CapPeriod::Custom {
                    anchor,
                    days: row.cap_period_days.unwrap_or(1).max(1) as u32,
                },
                _ => return None,
            };
            Some(Cap { amount, period })
        }
        _ => None,
    };
    Some(Rule {
        id: row.id,
        destination_id: row.destination_id,
        amount,
        source,
        customer_id: row.customer_id,
        active_from: row.active_from,
        active_to: row.active_to,
        max_amount: row.max_amount,
        cap,
        stop_at_target: row.stop_at_target,
    })
}

/// One line of a plan or of an adjusted allocation, with the destination's balance before and after.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlanLine {
    /// The rule that proposed it (none for an amount the user added).
    pub rule_id: Option<Uuid>,
    pub destination_id: Uuid,
    pub destination_name: String,
    /// What the rule asked for before limits (minor units).
    pub requested: i64,
    /// Minor units.
    pub amount: i64,
    pub limited_by: Option<Limit>,
    pub balance: i64,
    pub balance_after: i64,
    pub target_amount: Option<i64>,
}

/// Amount received, planned allocations and what remains (all minor units).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlanView {
    pub received_amount: i64,
    pub allocated: i64,
    pub remaining: i64,
    pub currency: String,
    pub lines: Vec<PlanLine>,
}

struct Books {
    destinations: HashMap<Uuid, DestinationRow>,
    balances: HashMap<Uuid, i64>,
}

async fn books(conn: &mut PgConnection) -> AppResult<Books> {
    let destinations = repo::list_destinations(conn)
        .await?
        .into_iter()
        .map(|d| (d.id, d))
        .collect();
    let balances = repo::balances(conn).await?.into_iter().collect();
    Ok(Books {
        destinations,
        balances,
    })
}

impl Books {
    fn line(
        &self,
        rule_id: Option<Uuid>,
        destination_id: Uuid,
        requested: i64,
        amount: i64,
        limited_by: Option<Limit>,
        added_before: i64,
    ) -> PlanLine {
        let destination = self.destinations.get(&destination_id);
        let balance = self.balances.get(&destination_id).copied().unwrap_or(0) + added_before;
        PlanLine {
            rule_id,
            destination_id,
            destination_name: destination.map(|d| d.name.clone()).unwrap_or_default(),
            requested,
            amount,
            limited_by,
            balance,
            balance_after: balance + amount,
            target_amount: destination.and_then(|d| d.target_amount),
        }
    }
}

/// The plan for a receipt by the active rules. `allocation_id` is left out of cap usage.
async fn plan_receipt(
    conn: &mut PgConnection,
    receipt: &Receipt,
    allocation_id: Uuid,
    currency: &str,
) -> AppResult<PlanView> {
    let books = books(conn).await?;
    let rules: Vec<Rule> = repo::list_rules(conn)
        .await?
        .iter()
        .filter(|r| {
            !r.archived
                && books
                    .destinations
                    .get(&r.destination_id)
                    .is_some_and(|d| !d.archived)
        })
        .filter_map(domain_rule)
        .collect();
    let mut context = PlanContext {
        balances: books.balances.clone(),
        targets: books
            .destinations
            .values()
            .filter_map(|d| d.target_amount.map(|t| (d.id, t)))
            .collect(),
        cap_used: HashMap::new(),
    };
    for rule in rules.iter().filter(|r| r.applies_to(receipt)) {
        if let Some(cap) = rule.cap {
            let (from, to) = period_containing(cap.period, receipt.date);
            let used = repo::rule_total_between(conn, rule.id, from, to, allocation_id).await?;
            context.cap_used.insert(rule.id, used);
        }
    }
    let plan = calc::plan(receipt, &rules, &context);
    let mut added: HashMap<Uuid, i64> = HashMap::new();
    let lines = plan
        .lines
        .iter()
        .map(|l| {
            let before = added.get(&l.destination_id).copied().unwrap_or(0);
            *added.entry(l.destination_id).or_default() += l.amount;
            books.line(
                Some(l.rule_id),
                l.destination_id,
                l.requested,
                l.amount,
                l.limited_by,
                before,
            )
        })
        .collect();
    Ok(PlanView {
        received_amount: receipt.amount,
        allocated: plan.allocated,
        remaining: plan.remaining,
        currency: currency.into(),
        lines,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    InvoicePayment,
    Income,
}

/// A receipt to try the rules on, without recording anything.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PreviewInput {
    /// Minor units.
    pub amount: i64,
    pub date: NaiveDate,
    pub source: ReceiptKind,
    #[serde(default)]
    pub income_type: Option<IncomeType>,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
}

pub async fn preview(
    state: &AppState,
    ctx: &BusinessContext,
    input: PreviewInput,
) -> AppResult<PlanView> {
    let mut errors = FieldErrors::new();
    check_amount(Some(input.amount), "amount", &mut errors);
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let currency = currency(&mut tx, ctx).await?;
    let receipt = Receipt {
        amount: input.amount,
        date: input.date,
        source: match input.source {
            ReceiptKind::InvoicePayment => ReceiptSource::InvoicePayment,
            ReceiptKind::Income => ReceiptSource::Income {
                sales: input.income_type != Some(IncomeType::Other),
            },
        },
        customer_id: input.customer_id,
    };
    let plan = plan_receipt(&mut tx, &receipt, Uuid::nil(), currency.code).await?;
    tx.commit().await?;
    Ok(plan)
}

// ─── Allocations ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AllocationDetail {
    #[serde(flatten)]
    pub allocation: AllocationRow,
    /// Confirmed entries and their reversals.
    pub entries: Vec<EntryRow>,
    /// What the rules propose now (pending or skipped allocations only).
    pub plan: Option<PlanView>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AllocationPage {
    pub items: Vec<AllocationDetail>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
}

fn receipt_of(row: &AllocationRow) -> Receipt {
    Receipt {
        amount: row.received_amount,
        date: row.received_date,
        source: if row.source_type == "income" {
            ReceiptSource::Income {
                sales: row.income_type.as_deref() != Some("other"),
            }
        } else {
            ReceiptSource::InvoicePayment
        },
        customer_id: row.customer_id,
    }
}

async fn detail(conn: &mut PgConnection, id: Uuid) -> AppResult<AllocationDetail> {
    let row = repo::find_allocation(conn, id)
        .await?
        .ok_or(AppError::NotFound("Allocation"))?;
    let entries = repo::entries(conn, &[id]).await?;
    let plan = if matches!(row.status.as_str(), "pending" | "skipped") {
        Some(plan_receipt(conn, &receipt_of(&row), row.id, &row.currency.clone()).await?)
    } else {
        None
    };
    Ok(AllocationDetail {
        allocation: row,
        entries,
        plan,
    })
}

pub async fn list(
    state: &AppState,
    ctx: &BusinessContext,
    filter: AllocationFilter,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<AllocationDetail>, i64)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::list_allocations(&mut tx, &filter, limit, offset).await?;
    let total = repo::count_allocations(&mut tx, &filter).await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut entries: HashMap<Uuid, Vec<EntryRow>> = HashMap::new();
    for entry in repo::entries(&mut tx, &ids).await? {
        entries.entry(entry.allocation_id).or_default().push(entry);
    }
    tx.commit().await?;
    let items = rows
        .into_iter()
        .map(|row| AllocationDetail {
            entries: entries.remove(&row.id).unwrap_or_default(),
            allocation: row,
            plan: None,
        })
        .collect();
    Ok((items, total))
}

pub async fn get(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<AllocationDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let detail = detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(detail)
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReviewLine {
    pub destination_id: Uuid,
    /// Minor units; zero lines are left out.
    pub amount: i64,
    /// The rule that proposed it, if any.
    #[serde(default)]
    pub rule_id: Option<Uuid>,
}

/// The amounts to record for an allocation, as reviewed by the user.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReviewInput {
    pub lines: Vec<ReviewLine>,
}

/// Validates reviewed lines against the receipt and returns them as a plan view (zero lines removed).
async fn check_lines(
    conn: &mut PgConnection,
    row: &AllocationRow,
    input: &ReviewInput,
) -> AppResult<PlanView> {
    let books = books(conn).await?;
    let rules: HashMap<Uuid, RuleRow> = repo::list_rules(conn)
        .await?
        .into_iter()
        .map(|r| (r.id, r))
        .collect();
    let mut errors = FieldErrors::new();
    let mut seen = Vec::new();
    let mut lines = Vec::new();
    let mut total: i64 = 0;
    for (i, line) in input.lines.iter().enumerate() {
        let field = |name: &str| format!("lines.{i}.{name}");
        if line.amount < 0 || line.amount > MAX_AMOUNT {
            errors.add(field("amount"), "Enter zero or more.");
            continue;
        }
        match books.destinations.get(&line.destination_id) {
            None => errors.add(field("destination_id"), "Choose a destination."),
            Some(d) if d.archived => errors.add(
                field("destination_id"),
                "This destination is archived. Choose another.",
            ),
            Some(_) if seen.contains(&line.destination_id) => errors.add(
                field("destination_id"),
                "This destination is already in the list.",
            ),
            Some(_) => seen.push(line.destination_id),
        }
        if line.rule_id.is_some_and(|r| !rules.contains_key(&r)) {
            errors.add(field("rule_id"), "This rule no longer exists.");
        }
        total = total.saturating_add(line.amount);
        if line.amount > 0 {
            lines.push(books.line(
                line.rule_id,
                line.destination_id,
                line.amount,
                line.amount,
                None,
                0,
            ));
        }
    }
    if total > row.received_amount {
        errors.add(
            "lines",
            "The allocations add up to more than the amount received.",
        );
    }
    errors.into_result()?;
    Ok(PlanView {
        received_amount: row.received_amount,
        allocated: total,
        remaining: row.received_amount - total,
        currency: row.currency.clone(),
        lines,
    })
}

async fn find_open(conn: &mut PgConnection, id: Uuid) -> AppResult<AllocationRow> {
    if !repo::lock_allocation(conn, id).await? {
        return Err(AppError::NotFound("Allocation"));
    }
    repo::find_allocation(conn, id)
        .await?
        .ok_or(AppError::NotFound("Allocation"))
}

/// Totals for reviewed lines, without recording them (for the review form).
pub async fn calculate(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: ReviewInput,
) -> AppResult<PlanView> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::find_allocation(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Allocation"))?;
    let view = check_lines(&mut tx, &row, &input).await?;
    tx.commit().await?;
    Ok(view)
}

fn money(row: &AllocationRow, amount: i64) -> String {
    Currency::from_code(&row.currency).map_or_else(|| amount.to_string(), |c| c.format(amount))
}

pub async fn confirm(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: ReviewInput,
) -> AppResult<AllocationDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = find_open(&mut tx, id).await?;
    if !matches!(row.status.as_str(), "pending" | "skipped") {
        return Err(AppError::Conflict(
            "This allocation is already confirmed or closed.".into(),
        ));
    }
    let view = check_lines(&mut tx, &row, &input).await?;
    for line in &view.lines {
        repo::insert_entry(
            &mut tx,
            ctx.business_id,
            id,
            line.destination_id,
            line.rule_id,
            line.amount,
            ctx.user_id(),
        )
        .await?;
    }
    repo::mark_confirmed(&mut tx, id, ctx.user_id()).await?;
    audit_record(
        &mut tx,
        ctx,
        "allocation.confirmed",
        "allocation",
        id,
        format!(
            "Allocated {} of {} received ({})",
            money(&row, view.allocated),
            money(&row, row.received_amount),
            row.source_label
        ),
        serde_json::json!({
            "lines": view.lines.iter().map(|l| serde_json::json!({ "destination_id": l.destination_id, "amount": l.amount, "rule_id": l.rule_id })).collect::<Vec<_>>(),
            "remaining": view.remaining,
        }),
    )
    .await?;
    let detail = detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(detail)
}

/// Skips a pending allocation (`skip = true`) or reopens a skipped one for review.
pub async fn set_skipped(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    skip: bool,
) -> AppResult<AllocationDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = find_open(&mut tx, id).await?;
    let (from, to) = if skip {
        ("pending", "skipped")
    } else {
        ("skipped", "pending")
    };
    if row.status != from {
        return Err(AppError::Conflict(if skip {
            "Only an allocation waiting for review can be skipped.".into()
        } else {
            "Only a skipped allocation can be reviewed again.".into()
        }));
    }
    repo::set_status(&mut tx, id, to).await?;
    audit_record(
        &mut tx,
        ctx,
        if skip {
            "allocation.skipped"
        } else {
            "allocation.reopened"
        },
        "allocation",
        id,
        format!(
            "Allocation of {} {} ({})",
            money(&row, row.received_amount),
            if skip { "skipped" } else { "reopened" },
            row.source_label
        ),
        serde_json::json!({}),
    )
    .await?;
    let detail = detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(detail)
}

/// Reverses a confirmed allocation's entries and closes it.
async fn reverse_confirmed(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    row: &AllocationRow,
    reason: &str,
) -> AppResult<()> {
    repo::reverse_entries(conn, row.id, ctx.user_id()).await?;
    repo::close(conn, row.id, "reversed", reason).await?;
    audit_record(
        conn,
        ctx,
        "allocation.reversed",
        "allocation",
        row.id,
        format!(
            "Allocation of {} reversed ({}): {reason}",
            money(row, row.received_amount),
            row.source_label
        ),
        serde_json::json!({ "allocated": row.allocated, "reason": reason }),
    )
    .await
}

/// The user's correction: reverses a confirmed allocation and opens a new one for review.
pub async fn reverse(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    reason: String,
) -> AppResult<AllocationDetail> {
    let mut errors = FieldErrors::new();
    let mut reason = clean(&reason, 500, "reason", &mut errors);
    errors.into_result()?;
    if reason.is_empty() {
        reason = "Reversed to correct it".into();
    }
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = find_open(&mut tx, id).await?;
    if row.status != "confirmed" {
        return Err(AppError::Conflict(
            "Only a confirmed allocation can be reversed.".into(),
        ));
    }
    reverse_confirmed(&mut tx, ctx, &row, &reason).await?;
    let source = source_of(&row)?;
    let new_id = repo::insert_allocation(
        &mut tx,
        ctx.business_id,
        source,
        row.received_date,
        row.received_amount,
        &row.currency,
        Some(row.id),
    )
    .await?;
    let detail = detail(&mut tx, new_id).await?;
    tx.commit().await?;
    Ok(detail)
}

fn source_of(row: &AllocationRow) -> AppResult<SourceRef> {
    match (row.invoice_payment_id, row.income_id) {
        (Some(id), None) => Ok(SourceRef::InvoicePayment(id)),
        (None, Some(id)) => Ok(SourceRef::Income(id)),
        _ => Err(AppError::internal(anyhow::anyhow!(
            "allocation {} has no single source",
            row.id
        ))),
    }
}

// ─── Balances ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DestinationBalance {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub archived: bool,
    /// Minor units.
    pub balance: i64,
    pub target_amount: Option<i64>,
    /// Balance as a whole percentage of the target (not capped at 100).
    pub target_progress: Option<i64>,
    pub minimum_balance: Option<i64>,
    pub below_minimum: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Balances {
    pub currency: String,
    /// Active destinations, then archived ones that still hold a balance.
    pub destinations: Vec<DestinationBalance>,
    /// Sum of all balances.
    pub total: i64,
    /// Allocations waiting for review.
    pub pending_count: i64,
    /// Whether any rule is active (allocations are only proposed when one is).
    pub has_rules: bool,
}

pub async fn balances(state: &AppState, ctx: &BusinessContext) -> AppResult<Balances> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let currency = currency(&mut tx, ctx).await?;
    let destinations: Vec<DestinationBalance> = repo::list_destinations(&mut tx)
        .await?
        .into_iter()
        .filter(|d| !d.archived || d.balance != 0)
        .map(|d| DestinationBalance {
            target_progress: d
                .target_amount
                .map(|t| (i128::from(d.balance.max(0)) * 100 / i128::from(t)) as i64),
            below_minimum: d.minimum_balance.is_some_and(|m| d.balance < m),
            id: d.id,
            name: d.name,
            kind: d.kind,
            archived: d.archived,
            balance: d.balance,
            target_amount: d.target_amount,
            minimum_balance: d.minimum_balance,
        })
        .collect();
    let pending_count = repo::pending_count(&mut tx).await?;
    let has_rules = repo::active_rule_count(&mut tx).await? > 0;
    tx.commit().await?;
    Ok(Balances {
        currency: currency.code.into(),
        total: destinations.iter().map(|d| d.balance).sum(),
        destinations,
        pending_count,
        has_rules,
    })
}

// ─── Receipts (called by income and invoice payments) ───────────────────────

/// A receipt as recorded, for keeping its allocation in step.
#[derive(Debug, Clone, Copy)]
pub struct ReceiptInfo {
    pub source: SourceRef,
    pub amount: i64,
    pub date: NaiveDate,
}

/// A receipt was recorded: opens an allocation for review when the business has rules.
pub async fn receipt_recorded(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    receipt: ReceiptInfo,
) -> AppResult<()> {
    if repo::active_rule_count(conn).await? == 0 {
        return Ok(());
    }
    let currency = currency(conn, ctx).await?;
    repo::insert_allocation(
        conn,
        ctx.business_id,
        receipt.source,
        receipt.date,
        receipt.amount,
        currency.code,
        None,
    )
    .await?;
    Ok(())
}

/// A receipt changed. An open allocation follows its date; when the amount of a confirmed one
/// changes, its entries are reversed and a new allocation opens for review.
pub async fn receipt_changed(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    receipt: ReceiptInfo,
) -> AppResult<()> {
    let Some((id, status, amount)) = repo::open_allocation_for(conn, receipt.source).await? else {
        return Ok(());
    };
    if status == "confirmed" && amount != receipt.amount {
        let row = repo::find_allocation(conn, id)
            .await?
            .ok_or(AppError::NotFound("Allocation"))?;
        let reason = format!(
            "Amount received changed from {} to {}",
            money(&row, amount),
            money(&row, receipt.amount)
        );
        reverse_confirmed(conn, ctx, &row, &reason).await?;
        repo::insert_allocation(
            conn,
            ctx.business_id,
            receipt.source,
            receipt.date,
            receipt.amount,
            &row.currency,
            Some(id),
        )
        .await?;
    } else {
        repo::update_received(conn, id, receipt.date, receipt.amount).await?;
    }
    Ok(())
}

/// A receipt was deleted: a confirmed allocation is reversed, an unconfirmed one cancelled.
pub async fn receipt_removed(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    source: SourceRef,
    reason: &str,
) -> AppResult<()> {
    let Some((id, status, _)) = repo::open_allocation_for(conn, source).await? else {
        return Ok(());
    };
    if status == "confirmed" {
        let row = repo::find_allocation(conn, id)
            .await?
            .ok_or(AppError::NotFound("Allocation"))?;
        reverse_confirmed(conn, ctx, &row, reason).await?;
    } else {
        repo::close(conn, id, "cancelled", reason).await?;
    }
    Ok(())
}

// ─── Spending from a destination (SL-408) ───────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SpendingInput {
    pub destination_id: Uuid,
    pub spent_on: NaiveDate,
    /// Minor units.
    pub amount: i64,
    pub description: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SpendingQuery {
    pub destination_id: Option<Uuid>,
    /// Include deleted spending.
    #[serde(default)]
    pub include_deleted: bool,
}

pub async fn list_spending(
    state: &AppState,
    ctx: &BusinessContext,
    query: SpendingQuery,
) -> AppResult<Vec<repo::SpendingRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::list_spending(
        &mut tx,
        query.destination_id,
        None,
        None,
        query.include_deleted,
    )
    .await?;
    tx.commit().await?;
    Ok(rows)
}

/// Records money used from a destination. It can't take more than the destination holds.
pub async fn record_spending(
    state: &AppState,
    ctx: &BusinessContext,
    input: SpendingInput,
) -> AppResult<repo::SpendingRow> {
    let mut errors = FieldErrors::new();
    let description = clean(&input.description, 200, "description", &mut errors);
    errors.check(
        !description.is_empty(),
        "description",
        "Say what it was spent on.",
    );
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    errors.check(
        input.amount > 0 && input.amount <= 1_000_000_000_000,
        "amount",
        "Enter the amount spent.",
    );
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let currency = currency(&mut tx, ctx).await?;
    let destination = repo::list_destinations(&mut tx)
        .await?
        .into_iter()
        .find(|d| d.id == input.destination_id)
        .ok_or_else(|| {
            AppError::validation("destination_id", "Choose where the money came from.")
        })?;
    if input.amount > destination.balance {
        return Err(AppError::validation(
            "amount",
            &format!(
                "{} holds {}: you can’t spend more than is set aside.",
                destination.name,
                currency.format(destination.balance.max(0))
            ),
        ));
    }
    let fields = repo::SpendingFields {
        destination_id: input.destination_id,
        spent_on: input.spent_on,
        amount: input.amount,
        description,
        notes,
    };
    let id = repo::insert_spending(&mut tx, ctx.business_id, ctx.user_id(), &fields).await?;
    audit_record(
        &mut tx,
        ctx,
        "allocation.spent",
        "allocation_spending",
        id,
        format!(
            "{} spent from {}: {}",
            currency.format(fields.amount),
            destination.name,
            fields.description
        ),
        serde_json::json!({ "destination_id": fields.destination_id, "amount": fields.amount, "spent_on": fields.spent_on }),
    )
    .await?;
    let row = repo::find_spending(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Spending"))?;
    tx.commit().await?;
    Ok(row)
}

/// Deletes spending (it stays in the history with the reason); the money is back in the balance.
pub async fn delete_spending(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    reason: String,
) -> AppResult<()> {
    let mut errors = FieldErrors::new();
    let reason = clean(&reason, 500, "reason", &mut errors);
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::find_spending(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Spending"))?;
    if row.voided {
        return Err(AppError::Conflict(
            "This spending is already deleted.".into(),
        ));
    }
    repo::void_spending(&mut tx, id, &reason).await?;
    audit_record(
        &mut tx,
        ctx,
        "allocation.spending_deleted",
        "allocation_spending",
        id,
        format!(
            "Spending from {} deleted: {}",
            row.destination_name, row.description
        ),
        serde_json::json!({ "reason": reason, "amount": row.amount }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

// ─── Allocating money received before the rules (SL-409) ────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BackfillInput {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BackfillResult {
    /// Allocations opened for review.
    pub opened: i64,
}

/// Opens an allocation for review for each payment and income received in the period that never had
/// one (recorded before any rule was active). Each receipt gets one at most, once.
pub async fn backfill(
    state: &AppState,
    ctx: &BusinessContext,
    input: BackfillInput,
) -> AppResult<BackfillResult> {
    if input.to < input.from {
        return Err(AppError::validation(
            "to",
            "The end date can’t be before the start date.",
        ));
    }
    if (input.to - input.from).num_days() > 366 * 2 {
        return Err(AppError::validation(
            "from",
            "Choose a period of up to two years.",
        ));
    }
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    if repo::active_rule_count(&mut tx).await? == 0 {
        return Err(AppError::Conflict(
            "Add a rule first: allocations are worked out from your rules.".into(),
        ));
    }
    let currency = currency(&mut tx, ctx).await?;
    let receipts = repo::receipts_without_allocation(&mut tx, input.from, input.to).await?;
    for (kind, id, date, amount) in &receipts {
        let source = if kind == "income" {
            SourceRef::Income(*id)
        } else {
            SourceRef::InvoicePayment(*id)
        };
        repo::insert_allocation(
            &mut tx,
            ctx.business_id,
            source,
            *date,
            *amount,
            currency.code,
            None,
        )
        .await?;
    }
    let opened = receipts.len() as i64;
    if opened > 0 {
        audit_record(
            &mut tx,
            ctx,
            "allocation.backfilled",
            "allocation",
            ctx.business_id,
            format!("{opened} earlier payments opened for allocation"),
            serde_json::json!({ "from": input.from, "to": input.to, "opened": opened }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(BackfillResult { opened })
}
