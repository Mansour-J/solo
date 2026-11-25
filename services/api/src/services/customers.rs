//! Customers: create, update, deactivate, delete (only without records), list and history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::Address;
use crate::domain::validation;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::businesses;
use crate::repositories::customers::{
    self, CustomerFields, CustomerRow, CustomerStats, ListFilter,
};
use crate::services::businesses::{business_today, diff};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CustomerInput {
    /// Name shown in lists and on documents (a person or an organisation).
    pub name: String,
    #[serde(default)]
    pub business_name: String,
    #[serde(default)]
    pub contact_person: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub billing_address: Address,
    #[serde(default)]
    pub business_address: Address,
    #[serde(default)]
    pub nzbn: Option<String>,
    #[serde(default)]
    pub tax_number: Option<String>,
    #[serde(default)]
    pub notes: String,
    /// Days until invoices are due; `null` uses the business default.
    #[serde(default)]
    pub payment_terms_days: Option<i32>,
    #[serde(default)]
    pub default_currency: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Customer {
    pub id: Uuid,
    pub name: String,
    pub business_name: String,
    pub contact_person: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub billing_address: Address,
    pub business_address: Address,
    pub nzbn: Option<String>,
    pub tax_number: Option<String>,
    pub notes: String,
    pub payment_terms_days: Option<i32>,
    pub default_currency: Option<String>,
    pub is_active: bool,
    /// Invoice reminders go to this customer (when reminders are on).
    pub send_reminders: bool,
    /// Unpaid amount on issued invoices (minor units).
    pub outstanding: i64,
    /// Part of `outstanding` that is past its due date.
    pub overdue: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<CustomerRow> for Customer {
    fn from(r: CustomerRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            business_name: r.business_name,
            contact_person: r.contact_person,
            email: r.email,
            phone: r.phone,
            billing_address: serde_json::from_value(r.billing_address).unwrap_or_default(),
            business_address: serde_json::from_value(r.business_address).unwrap_or_default(),
            nzbn: r.nzbn,
            tax_number: r.tax_number,
            notes: r.notes,
            payment_terms_days: r.payment_terms_days,
            default_currency: r.default_currency,
            is_active: r.is_active,
            send_reminders: r.send_reminders,
            outstanding: r.outstanding,
            overdue: r.overdue,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CustomerDetail {
    #[serde(flatten)]
    pub customer: Customer,
    pub stats: CustomerStats,
}

fn optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

fn validate(input: CustomerInput) -> AppResult<CustomerFields> {
    let mut errors = FieldErrors::new();
    let name = input.name.trim().to_owned();
    errors.check(!name.is_empty(), "name", "Enter the customer’s name.");
    errors.check(
        name.chars().count() <= 200,
        "name",
        "Use 200 characters or fewer.",
    );
    for (field, value, max) in [
        ("business_name", &input.business_name, 200),
        ("contact_person", &input.contact_person, 200),
        ("notes", &input.notes, 4000),
    ] {
        errors.check(
            value.trim().chars().count() <= max,
            field,
            "This is too long.",
        );
    }
    let email = optional(input.email).map(|e| validation::normalize_email(&e));
    if let Some(email) = &email {
        errors.check(
            validation::is_valid_email(email),
            "email",
            "Enter a valid email address.",
        );
    }
    let phone = optional(input.phone);
    if let Some(phone) = &phone {
        errors.check(
            phone.chars().count() <= 40
                && phone
                    .chars()
                    .all(|c| c.is_ascii_digit() || " +()-.".contains(c)),
            "phone",
            "Enter a phone number using digits, spaces and + ( ) - only.",
        );
    }
    let nzbn = optional(input.nzbn).map(|n| validation::digits_only(&n));
    if let Some(nzbn) = &nzbn {
        errors.check(
            validation::is_valid_nzbn(nzbn),
            "nzbn",
            "Enter a valid 13-digit NZBN.",
        );
    }
    let tax_number = optional(input.tax_number);
    errors.check(
        tax_number.as_ref().is_none_or(|t| t.chars().count() <= 40),
        "tax_number",
        "Use 40 characters or fewer.",
    );
    if let Some(days) = input.payment_terms_days {
        errors.check(
            (0..=365).contains(&days),
            "payment_terms_days",
            "Use 0 to 365 days.",
        );
    }
    let currency = optional(input.default_currency).map(|c| c.to_uppercase());
    if let Some(currency) = &currency {
        errors.check(
            validation::is_supported_currency(currency),
            "default_currency",
            "Choose a supported currency.",
        );
    }
    let billing = input.billing_address.trimmed();
    let business = input.business_address.trimmed();
    for (prefix, address) in [
        ("billing_address", &billing),
        ("business_address", &business),
    ] {
        for (field, value) in address.fields() {
            errors.check(
                value.chars().count() <= Address::MAX_FIELD,
                &format!("{prefix}.{field}"),
                "Use 120 characters or fewer.",
            );
        }
    }
    errors.into_result()?;
    Ok(CustomerFields {
        name,
        business_name: input.business_name.trim().to_owned(),
        contact_person: input.contact_person.trim().to_owned(),
        email,
        phone,
        billing_address: serde_json::to_value(billing).map_err(AppError::internal)?,
        business_address: serde_json::to_value(business).map_err(AppError::internal)?,
        nzbn,
        tax_number,
        notes: input.notes.trim().to_owned(),
        payment_terms_days: input.payment_terms_days,
        default_currency: currency,
        is_active: input.is_active,
    })
}

/// Today in the business's time zone (for overdue calculations).
pub async fn today(
    conn: &mut sqlx::PgConnection,
    business_id: Uuid,
) -> AppResult<chrono::NaiveDate> {
    let business = businesses::find(conn, business_id)
        .await?
        .ok_or(AppError::NotFound("Business"))?;
    Ok(business_today(&business.timezone))
}

pub async fn list(
    state: &AppState,
    ctx: &BusinessContext,
    filter: ListFilter,
) -> AppResult<(Vec<Customer>, i64)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let today = today(&mut tx, ctx.business_id).await?;
    let (rows, total) = customers::list(&mut tx, today, &filter).await?;
    tx.commit().await?;
    Ok((rows.into_iter().map(Customer::from).collect(), total))
}

pub async fn get(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<CustomerDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let today = today(&mut tx, ctx.business_id).await?;
    let row = customers::find(&mut tx, today, id)
        .await?
        .ok_or(AppError::NotFound("Customer"))?;
    let stats = customers::stats(&mut tx, id).await?;
    tx.commit().await?;
    Ok(CustomerDetail {
        customer: row.into(),
        stats,
    })
}

fn snapshot(f: &CustomerFields) -> serde_json::Value {
    serde_json::json!({
        "name": f.name, "business_name": f.business_name, "contact_person": f.contact_person, "email": f.email,
        "phone": f.phone, "billing_address": f.billing_address, "business_address": f.business_address, "nzbn": f.nzbn,
        "tax_number": f.tax_number, "notes": f.notes, "payment_terms_days": f.payment_terms_days,
        "default_currency": f.default_currency, "is_active": f.is_active,
    })
}

pub async fn create(
    state: &AppState,
    ctx: &BusinessContext,
    input: CustomerInput,
) -> AppResult<CustomerDetail> {
    let fields = validate(input)?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let id = customers::insert(&mut tx, ctx.business_id, &fields).await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "customer.created",
            entity_type: "customer",
            entity_id: Some(id),
            summary: format!("Customer “{}” added", fields.name),
            changes: serde_json::json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    get(state, ctx, id).await
}

pub async fn update(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: CustomerInput,
) -> AppResult<CustomerDetail> {
    let fields = validate(input)?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let today = today(&mut tx, ctx.business_id).await?;
    let before = customers::find(&mut tx, today, id)
        .await?
        .ok_or(AppError::NotFound("Customer"))?;
    let before_fields = CustomerFields {
        name: before.name.clone(),
        business_name: before.business_name.clone(),
        contact_person: before.contact_person.clone(),
        email: before.email.clone(),
        phone: before.phone.clone(),
        billing_address: before.billing_address.clone(),
        business_address: before.business_address.clone(),
        nzbn: before.nzbn.clone(),
        tax_number: before.tax_number.clone(),
        notes: before.notes.clone(),
        payment_terms_days: before.payment_terms_days,
        default_currency: before.default_currency.clone(),
        is_active: before.is_active,
    };
    customers::update(&mut tx, id, &fields).await?;
    let changes = diff(&snapshot(&before_fields), &snapshot(&fields));
    if changes.as_object().is_some_and(|c| !c.is_empty()) {
        audit::record(
            &mut tx,
            &ctx.actor(),
            audit::Entry {
                business_id: Some(ctx.business_id),
                action: "customer.updated",
                entity_type: "customer",
                entity_id: Some(id),
                summary: format!("Customer “{}” updated", fields.name),
                changes,
            },
        )
        .await?;
    }
    tx.commit().await?;
    get(state, ctx, id).await
}

pub async fn delete(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let today = today(&mut tx, ctx.business_id).await?;
    let customer = customers::find(&mut tx, today, id)
        .await?
        .ok_or(AppError::NotFound("Customer"))?;
    if customers::has_records(&mut tx, id).await? {
        return Err(AppError::Conflict(
            "This customer has invoices or other records, so it can’t be deleted. Mark it inactive instead.".into(),
        ));
    }
    customers::delete(&mut tx, id).await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "customer.deleted",
            entity_type: "customer",
            entity_id: Some(id),
            summary: format!("Customer “{}” deleted", customer.name),
            changes: serde_json::json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
