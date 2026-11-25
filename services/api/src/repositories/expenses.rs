//! Expense categories, expenses, receipt attachments and income records.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::domain::tax::TaxTreatment;

// ─── Categories ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct CategoryRow {
    pub id: Uuid,
    pub name: String,
    pub account_id: Uuid,
    pub account_code: String,
    pub is_default: bool,
    /// Not deductible for tax (e.g. penalties): reports show it as an adjustment.
    pub non_deductible: bool,
    /// A direct cost of what's sold: reports show it as cost of sales, before gross profit.
    pub cost_of_sales: bool,
    pub archived: bool,
    /// Number of active expenses in the category.
    pub expense_count: i64,
}

pub async fn categories(conn: &mut PgConnection) -> Result<Vec<CategoryRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT c.id, c.name, c.account_id, a.code AS account_code, c.is_default, a.non_deductible, a.cost_of_sales, c.archived_at IS NOT NULL AS archived,
                (SELECT count(*) FROM expenses e WHERE e.category_id = c.id AND e.voided_at IS NULL) AS expense_count
         FROM expense_categories c JOIN financial_accounts a ON a.id = c.account_id
         ORDER BY c.archived_at IS NOT NULL, lower(c.name)",
    )
    .fetch_all(conn)
    .await
}

pub async fn find_category(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<CategoryRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT c.id, c.name, c.account_id, a.code AS account_code, c.is_default, a.non_deductible, a.cost_of_sales, c.archived_at IS NOT NULL AS archived,
                (SELECT count(*) FROM expenses e WHERE e.category_id = c.id AND e.voided_at IS NULL) AS expense_count
         FROM expense_categories c JOIN financial_accounts a ON a.id = c.account_id WHERE c.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub async fn category_name_taken(
    conn: &mut PgConnection,
    name: &str,
    except: Option<Uuid>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM expense_categories WHERE lower(name) = lower($1) AND archived_at IS NULL AND ($2::uuid IS NULL OR id <> $2))",
    )
    .bind(name)
    .bind(except)
    .fetch_one(conn)
    .await
}

/// Creates a category with its own expense account and returns its id, or `None` when every code in
/// the 5000–5999 range is taken. Codes go up in tens after the highest in use; once that passes 5999
/// the first free code is used. Codes from 6000 belong to other accounts (6000 is Mileage claims).
pub async fn insert_category(
    conn: &mut PgConnection,
    business_id: Uuid,
    name: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let code: Option<i32> = sqlx::query_scalar(
        "WITH used AS (SELECT code::int AS c FROM financial_accounts WHERE code ~ '^5[0-9]{3}$') \
         SELECT CASE WHEN coalesce(max(c), 4990) + 10 <= 5999 THEN coalesce(max(c), 4990) + 10 \
                ELSE (SELECT min(s) FROM generate_series(5000, 5999) AS s WHERE s NOT IN (SELECT c FROM used)) END \
         FROM used",
    )
    .fetch_one(&mut *conn)
    .await?;
    let Some(code) = code else {
        return Ok(None);
    };
    let account_id: Uuid = sqlx::query_scalar(
        "INSERT INTO financial_accounts (business_id, code, name, account_type) VALUES ($1, $2, $3, 'expense') RETURNING id",
    )
    .bind(business_id)
    .bind(code.to_string())
    .bind(name)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query_scalar("INSERT INTO expense_categories (business_id, name, account_id) VALUES ($1, $2, $3) RETURNING id")
        .bind(business_id)
        .bind(name)
        .bind(account_id)
        .fetch_one(conn)
        .await
        .map(Some)
}

pub async fn rename_category(
    conn: &mut PgConnection,
    id: Uuid,
    account_id: Uuid,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE expense_categories SET name = $2 WHERE id = $1")
        .bind(id)
        .bind(name)
        .execute(&mut *conn)
        .await?;
    sqlx::query("UPDATE financial_accounts SET name = $2 WHERE id = $1")
        .bind(account_id)
        .bind(name)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn set_category_archived(
    conn: &mut PgConnection,
    id: Uuid,
    archived: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE expense_categories SET archived_at = CASE WHEN $2 THEN now() END WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(conn)
    .await
    .map(|_| ())
}

// ─── Expenses ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseRow {
    pub id: Uuid,
    pub expense_date: NaiveDate,
    pub supplier: String,
    pub description: String,
    pub category_id: Uuid,
    pub category_name: String,
    pub account_id: Uuid,
    pub currency: String,
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    pub gst_amount: i64,
    pub net_amount: i64,
    pub payment_method: String,
    pub payment_reference: String,
    pub classification: String,
    pub business_percentage: Decimal,
    pub deductible_percentage: Decimal,
    pub rechargeable_customer_id: Option<Uuid>,
    pub rechargeable_customer_name: Option<String>,
    pub notes: String,
    pub attachment_count: i64,
    pub voided_at: Option<DateTime<Utc>>,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! expense_columns {
    () => {
        "e.id, e.expense_date, e.supplier, e.description, e.category_id, c.name AS category_name, c.account_id, e.currency,
         e.gross_amount, e.gst_treatment, e.gst_amount, e.net_amount, e.payment_method, e.payment_reference, e.classification,
         e.business_percentage, e.deductible_percentage, e.rechargeable_customer_id, cu.name AS rechargeable_customer_name, e.notes,
         (SELECT count(*) FROM expense_attachments a WHERE a.expense_id = e.id) AS attachment_count,
         e.voided_at, e.void_reason, e.created_at, e.updated_at"
    };
}

macro_rules! expense_from {
    () => {
        " FROM expenses e JOIN expense_categories c ON c.id = e.category_id LEFT JOIN customers cu ON cu.id = e.rechargeable_customer_id "
    };
}

pub struct ExpenseFields {
    pub expense_date: NaiveDate,
    pub supplier: String,
    pub description: String,
    pub category_id: Uuid,
    pub currency: String,
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    pub gst_amount: i64,
    pub payment_method: String,
    pub payment_reference: String,
    pub classification: String,
    pub business_percentage: Decimal,
    pub deductible_percentage: Decimal,
    pub rechargeable_customer_id: Option<Uuid>,
    pub notes: String,
}

pub async fn insert_expense(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    f: &ExpenseFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO expenses (business_id, expense_date, supplier, description, category_id, currency, gross_amount, gst_treatment,
            gst_amount, net_amount, payment_method, payment_reference, classification, business_percentage, deductible_percentage,
            rechargeable_customer_id, notes, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $7 - $9, $10, $11, $12, $13, $14, $15, $16, $17) RETURNING id",
    )
    .bind(business_id)
    .bind(f.expense_date)
    .bind(&f.supplier)
    .bind(&f.description)
    .bind(f.category_id)
    .bind(&f.currency)
    .bind(f.gross_amount)
    .bind(f.gst_treatment)
    .bind(f.gst_amount)
    .bind(&f.payment_method)
    .bind(&f.payment_reference)
    .bind(&f.classification)
    .bind(f.business_percentage)
    .bind(f.deductible_percentage)
    .bind(f.rechargeable_customer_id)
    .bind(&f.notes)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn update_expense(
    conn: &mut PgConnection,
    id: Uuid,
    f: &ExpenseFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE expenses SET expense_date = $2, supplier = $3, description = $4, category_id = $5, gross_amount = $6, gst_treatment = $7,
            gst_amount = $8, net_amount = $6 - $8, payment_method = $9, payment_reference = $10, classification = $11,
            business_percentage = $12, deductible_percentage = $13, rechargeable_customer_id = $14, notes = $15
         WHERE id = $1 AND voided_at IS NULL",
    )
    .bind(id)
    .bind(f.expense_date)
    .bind(&f.supplier)
    .bind(&f.description)
    .bind(f.category_id)
    .bind(f.gross_amount)
    .bind(f.gst_treatment)
    .bind(f.gst_amount)
    .bind(&f.payment_method)
    .bind(&f.payment_reference)
    .bind(&f.classification)
    .bind(f.business_percentage)
    .bind(f.deductible_percentage)
    .bind(f.rechargeable_customer_id)
    .bind(&f.notes)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn void_expense(
    conn: &mut PgConnection,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE expenses SET voided_at = now(), void_reason = $2 WHERE id = $1 AND voided_at IS NULL")
        .bind(id)
        .bind(reason)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn find_expense(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<ExpenseRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        expense_columns!(),
        expense_from!(),
        "WHERE e.id = $1"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub struct ExpenseFilter {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub category_id: Option<Uuid>,
    pub search: Option<String>,
    pub min_amount: Option<i64>,
    pub max_amount: Option<i64>,
    pub gst_treatment: Option<String>,
    pub has_receipt: Option<bool>,
    pub classification: Option<String>,
    pub include_voided: bool,
    pub limit: i64,
    pub offset: i64,
}

macro_rules! expense_where {
    () => {
        " WHERE ($1::date IS NULL OR e.expense_date >= $1) AND ($2::date IS NULL OR e.expense_date <= $2)
            AND ($3::uuid IS NULL OR e.category_id = $3)
            AND ($4::text IS NULL OR e.supplier ILIKE $4 OR e.description ILIKE $4 OR e.payment_reference ILIKE $4)
            AND ($5::bigint IS NULL OR e.gross_amount >= $5) AND ($6::bigint IS NULL OR e.gross_amount <= $6)
            AND ($7::text IS NULL OR e.gst_treatment = $7)
            AND ($8::boolean IS NULL OR EXISTS (SELECT 1 FROM expense_attachments a WHERE a.expense_id = e.id) = $8)
            AND ($9::text IS NULL OR e.classification = $9)
            AND ($10 OR e.voided_at IS NULL) "
    };
}

/// A page of expenses plus totals over every matching expense.
pub async fn list_expenses(
    conn: &mut PgConnection,
    f: &ExpenseFilter,
) -> Result<(Vec<ExpenseRow>, i64, i64, i64), sqlx::Error> {
    let search = f.search.as_ref().map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let rows: Vec<ExpenseRow> = sqlx::query_as(concat!(
        "SELECT ",
        expense_columns!(),
        expense_from!(),
        expense_where!(),
        "ORDER BY e.expense_date DESC, e.created_at DESC LIMIT $11 OFFSET $12"
    ))
    .bind(f.from)
    .bind(f.to)
    .bind(f.category_id)
    .bind(&search)
    .bind(f.min_amount)
    .bind(f.max_amount)
    .bind(&f.gst_treatment)
    .bind(f.has_receipt)
    .bind(&f.classification)
    .bind(f.include_voided)
    .bind(f.limit)
    .bind(f.offset)
    .fetch_all(&mut *conn)
    .await?;
    let (count, gross, gst): (i64, i64, i64) = sqlx::query_as(concat!(
        "SELECT count(*), coalesce(sum(e.gross_amount) FILTER (WHERE e.voided_at IS NULL), 0)::bigint,
                coalesce(sum(e.gst_amount) FILTER (WHERE e.voided_at IS NULL), 0)::bigint",
        expense_from!(),
        expense_where!()
    ))
    .bind(f.from)
    .bind(f.to)
    .bind(f.category_id)
    .bind(&search)
    .bind(f.min_amount)
    .bind(f.max_amount)
    .bind(&f.gst_treatment)
    .bind(f.has_receipt)
    .bind(&f.classification)
    .bind(f.include_voided)
    .fetch_one(conn)
    .await?;
    Ok((rows, count, gross, gst))
}

// ─── Attachments ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct AttachmentRow {
    pub id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i32,
    pub created_at: DateTime<Utc>,
    #[serde(skip)]
    pub object_key: String,
}

pub async fn attachments(
    conn: &mut PgConnection,
    expense_id: Uuid,
) -> Result<Vec<AttachmentRow>, sqlx::Error> {
    sqlx::query_as("SELECT id, filename, content_type, size_bytes, created_at, object_key FROM expense_attachments WHERE expense_id = $1 ORDER BY created_at")
        .bind(expense_id)
        .fetch_all(conn)
        .await
}

pub struct NewAttachment<'a> {
    pub business_id: Uuid,
    pub expense_id: Uuid,
    pub object_key: &'a str,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub size_bytes: i32,
    pub uploaded_by: Uuid,
}

pub async fn insert_attachment(
    conn: &mut PgConnection,
    a: NewAttachment<'_>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO expense_attachments (business_id, expense_id, object_key, filename, content_type, size_bytes, uploaded_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(a.business_id)
    .bind(a.expense_id)
    .bind(a.object_key)
    .bind(a.filename)
    .bind(a.content_type)
    .bind(a.size_bytes)
    .bind(a.uploaded_by)
    .fetch_one(conn)
    .await
}

pub async fn delete_attachment(conn: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM expense_attachments WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(|_| ())
}

/// Storage keys of every attachment in the current business (deleted with the business).
pub async fn all_attachment_keys(conn: &mut PgConnection) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT object_key FROM expense_attachments")
        .fetch_all(conn)
        .await
}

// ─── Income ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct IncomeRow {
    pub id: Uuid,
    pub income_date: NaiveDate,
    pub payer: String,
    pub description: String,
    pub income_type: String,
    pub customer_id: Option<Uuid>,
    pub customer_name: Option<String>,
    pub currency: String,
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    pub gst_amount: i64,
    pub net_amount: i64,
    pub payment_method: String,
    pub reference: String,
    pub notes: String,
    pub voided_at: Option<DateTime<Utc>>,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! income_columns {
    () => {
        "r.id, r.income_date, r.payer, r.description, r.income_type, r.customer_id, c.name AS customer_name, r.currency, r.gross_amount,
         r.gst_treatment, r.gst_amount, r.net_amount, r.payment_method, r.reference, r.notes, r.voided_at, r.void_reason,
         r.created_at, r.updated_at FROM income_records r LEFT JOIN customers c ON c.id = r.customer_id"
    };
}

pub struct IncomeFields {
    pub income_date: NaiveDate,
    pub payer: String,
    pub description: String,
    pub income_type: String,
    pub customer_id: Option<Uuid>,
    pub currency: String,
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    pub gst_amount: i64,
    pub payment_method: String,
    pub reference: String,
    pub notes: String,
}

pub async fn insert_income(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    f: &IncomeFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO income_records (business_id, income_date, payer, description, income_type, customer_id, currency, gross_amount,
            gst_treatment, gst_amount, net_amount, payment_method, reference, notes, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $8 - $10, $11, $12, $13, $14) RETURNING id",
    )
    .bind(business_id)
    .bind(f.income_date)
    .bind(&f.payer)
    .bind(&f.description)
    .bind(&f.income_type)
    .bind(f.customer_id)
    .bind(&f.currency)
    .bind(f.gross_amount)
    .bind(f.gst_treatment)
    .bind(f.gst_amount)
    .bind(&f.payment_method)
    .bind(&f.reference)
    .bind(&f.notes)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn update_income(
    conn: &mut PgConnection,
    id: Uuid,
    f: &IncomeFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE income_records SET income_date = $2, payer = $3, description = $4, income_type = $5, customer_id = $6, gross_amount = $7,
            gst_treatment = $8, gst_amount = $9, net_amount = $7 - $9, payment_method = $10, reference = $11, notes = $12
         WHERE id = $1 AND voided_at IS NULL",
    )
    .bind(id)
    .bind(f.income_date)
    .bind(&f.payer)
    .bind(&f.description)
    .bind(&f.income_type)
    .bind(f.customer_id)
    .bind(f.gross_amount)
    .bind(f.gst_treatment)
    .bind(f.gst_amount)
    .bind(&f.payment_method)
    .bind(&f.reference)
    .bind(&f.notes)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn void_income(
    conn: &mut PgConnection,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE income_records SET voided_at = now(), void_reason = $2 WHERE id = $1 AND voided_at IS NULL")
        .bind(id)
        .bind(reason)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn find_income(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<IncomeRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", income_columns!(), " WHERE r.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn list_income(
    conn: &mut PgConnection,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    search: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<IncomeRow>, i64, i64), sqlx::Error> {
    let search = search.map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let rows: Vec<IncomeRow> = sqlx::query_as(concat!(
        "SELECT ",
        income_columns!(),
        " WHERE r.voided_at IS NULL AND ($1::date IS NULL OR r.income_date >= $1) AND ($2::date IS NULL OR r.income_date <= $2)
            AND ($3::text IS NULL OR r.payer ILIKE $3 OR r.description ILIKE $3 OR r.reference ILIKE $3)
          ORDER BY r.income_date DESC, r.created_at DESC LIMIT $4 OFFSET $5"
    ))
    .bind(from)
    .bind(to)
    .bind(&search)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *conn)
    .await?;
    let (count, gross): (i64, i64) = sqlx::query_as(
        "SELECT count(*), coalesce(sum(r.gross_amount), 0)::bigint FROM income_records r
         WHERE r.voided_at IS NULL AND ($1::date IS NULL OR r.income_date >= $1) AND ($2::date IS NULL OR r.income_date <= $2)
           AND ($3::text IS NULL OR r.payer ILIKE $3 OR r.description ILIKE $3 OR r.reference ILIKE $3)",
    )
    .bind(from)
    .bind(to)
    .bind(&search)
    .fetch_one(conn)
    .await?;
    Ok((rows, count, gross))
}

/// Marks a category's account as cost of sales (or not).
pub async fn set_account_cost_of_sales(
    conn: &mut PgConnection,
    account_id: Uuid,
    cost_of_sales: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE financial_accounts SET cost_of_sales = $2 WHERE id = $1")
        .bind(account_id)
        .bind(cost_of_sales)
        .execute(conn)
        .await
        .map(|_| ())
}

/// Marks a category's account as not deductible for tax (or deductible again).
pub async fn set_account_non_deductible(
    conn: &mut PgConnection,
    account_id: Uuid,
    non_deductible: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE financial_accounts SET non_deductible = $2 WHERE id = $1")
        .bind(account_id)
        .bind(non_deductible)
        .execute(conn)
        .await
        .map(|_| ())
}
