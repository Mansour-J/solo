//! `/api/v1/businesses/{business_id}/reports/{kind}`: every report as JSON, and the same report as
//! CSV, Excel or PDF from one export layer.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::extract::ApiQuery;
use crate::context::BusinessContext;
use crate::domain::business::ReportingBasis;
use crate::domain::reports::Report;
use crate::error::{AppResult, ErrorBody};
use crate::services::report_export::{self, ExportFormat};
use crate::services::reports::{self, ReportKind, ReportParams};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ReportPath {
    #[allow(dead_code)]
    pub business_id: Uuid,
    pub kind: ReportKind,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllocationStatusFilter {
    Pending,
    Confirmed,
    Skipped,
    Reversed,
    Cancelled,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReportQuery {
    /// Defaults to the start of the current financial year.
    pub from: Option<NaiveDate>,
    /// Defaults to the end of the current financial year.
    pub to: Option<NaiveDate>,
    /// Defaults to the business's reporting basis.
    #[param(inline)]
    pub basis: Option<ReportingBasis>,
    /// Income summary, receivables; required for a statement of account.
    pub customer_id: Option<Uuid>,
    /// An expense category (expense summary).
    pub category_id: Option<Uuid>,
    /// Mileage report.
    pub vehicle_id: Option<Uuid>,
    /// Allocations report.
    #[param(inline)]
    pub status: Option<AllocationStatusFilter>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FormatQuery {
    #[param(inline)]
    pub format: ExportFormat,
}

impl From<ReportQuery> for ReportParams {
    fn from(q: ReportQuery) -> Self {
        Self {
            from: q.from,
            to: q.to,
            basis: q.basis,
            customer_id: q.customer_id,
            category_id: q.category_id,
            vehicle_id: q.vehicle_id,
            status: q.status.map(|s| {
                match s {
                    AllocationStatusFilter::Pending => "pending",
                    AllocationStatusFilter::Confirmed => "confirmed",
                    AllocationStatusFilter::Skipped => "skipped",
                    AllocationStatusFilter::Reversed => "reversed",
                    AllocationStatusFilter::Cancelled => "cancelled",
                }
                .to_owned()
            }),
        }
    }
}

/// A report: sections of typed columns and rows, with totals. Money is in minor units.
#[utoipa::path(get, path = "/{business_id}/reports/{kind}", tag = "reports",
    params(("business_id" = Uuid, Path), ("kind" = inline(ReportKind), Path), ReportQuery),
    responses((status = OK, body = Report), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn get_report(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ReportPath>,
    ApiQuery(q): ApiQuery<ReportQuery>,
) -> AppResult<Json<Report>> {
    Ok(Json(reports::build(&state, &ctx, p.kind, &q.into()).await?))
}

/// The same report as a file: CSV (UTF-8 with BOM), Excel or PDF.
#[utoipa::path(get, path = "/{business_id}/reports/{kind}/export", tag = "reports",
    params(("business_id" = Uuid, Path), ("kind" = inline(ReportKind), Path), FormatQuery, ReportQuery),
    responses((status = OK, content_type = "application/octet-stream", body = Vec<u8>), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn export_report(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ReportPath>,
    ApiQuery(f): ApiQuery<FormatQuery>,
    ApiQuery(q): ApiQuery<ReportQuery>,
) -> AppResult<impl IntoResponse> {
    let report = reports::build(&state, &ctx, p.kind, &q.into()).await?;
    let bytes = match f.format {
        ExportFormat::Csv => report_export::to_csv(&report),
        ExportFormat::Xlsx => report_export::to_xlsx(&report)?,
        ExportFormat::Pdf => {
            let brand = &state.config.brand;
            report_export::to_pdf(&report, &brand.primary_color, &brand.name).await?
        }
    };
    Ok((
        [
            (header::CONTENT_TYPE, f.format.content_type().to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{}\"",
                    report_export::file_name(&report, f.format)
                ),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    ))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExportAllQuery {
    /// Defaults to the start of the current financial year.
    pub from: Option<NaiveDate>,
    /// Defaults to the end of the current financial year.
    pub to: Option<NaiveDate>,
    /// Defaults to the business's reporting basis.
    #[param(inline)]
    pub basis: Option<ReportingBasis>,
}

/// Every report (except the statement of account, which is per customer) for one period and basis, in
/// one file: a PDF with each report on new pages, an Excel workbook with a sheet each, or one CSV.
#[utoipa::path(get, path = "/{business_id}/reports/all/export", tag = "reports",
    params(("business_id" = Uuid, Path), FormatQuery, ExportAllQuery),
    responses((status = OK, content_type = "application/octet-stream", body = Vec<u8>), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn export_all(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(f): ApiQuery<FormatQuery>,
    ApiQuery(q): ApiQuery<ExportAllQuery>,
) -> AppResult<impl IntoResponse> {
    let all = reports::build_all(&state, &ctx, q.from, q.to, q.basis).await?;
    let first = all.first().expect("EXPORT_ALL is not empty");
    let period = first
        .period
        .map(|p| format!(" {} to {}", p.from, p.to))
        .unwrap_or_default();
    let title = format!("All reports{period}");
    let bytes = match f.format {
        ExportFormat::Csv => report_export::to_csv_many(&all),
        ExportFormat::Xlsx => report_export::to_xlsx_many(&all)?,
        ExportFormat::Pdf => {
            let brand = &state.config.brand;
            report_export::to_pdf_many(&all, &title, &brand.primary_color, &brand.name).await?
        }
    };
    Ok((
        [
            (header::CONTENT_TYPE, f.format.content_type().to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{title}.{}\"", f.format.extension()),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    ))
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PackQuery {
    /// The last day of the financial year.
    pub year_end: chrono::NaiveDate,
    /// Defaults to the business's reporting basis.
    pub basis: Option<ReportingBasis>,
}

/// The accountant pack: a zip with every report for the financial year (PDF and Excel), the general
/// journal, the mileage log and the expense receipts.
#[utoipa::path(get, path = "/{business_id}/reports/accountant-pack", tag = "reports",
    params(("business_id" = Uuid, Path), PackQuery),
    responses((status = OK, content_type = "application/zip", body = Vec<u8>), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn accountant_pack(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<PackQuery>,
) -> AppResult<impl IntoResponse> {
    let (name, bytes) =
        crate::services::accountant_pack::build(&state, &ctx, q.year_end, q.basis).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    ))
}

/// Issued invoices or expenses for a period, as CSV in the format Xero imports.
#[utoipa::path(get, path = "/{business_id}/reports/xero-export", tag = "reports",
    params(("business_id" = Uuid, Path), crate::services::xero_export::XeroExportQuery),
    responses((status = OK, content_type = "text/csv", body = Vec<u8>), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn xero_export(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<crate::services::xero_export::XeroExportQuery>,
) -> AppResult<impl IntoResponse> {
    let (name, bytes) = crate::services::xero_export::export(&state, &ctx, q).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    ))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_report))
        .routes(routes!(export_report))
        .routes(routes!(export_all))
        .routes(routes!(accountant_pack))
        .routes(routes!(xero_export))
}
