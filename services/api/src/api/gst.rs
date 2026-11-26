//! `/api/v1/businesses/{business_id}/gst` and `/income-tax`: GST returns, income tax and provisional
//! tax, and what is owed to IRD.

use axum::Json;
use axum::extract::State;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::context::BusinessContext;
use crate::error::AppResult;
use crate::services::gst_returns::{self, GstReturnBoxes, GstReturns};
use crate::services::income_tax::{self, AccSettings, IncomeTax};
use crate::state::AppState;

/// Every GST return period from the first GST record to today, with its IRD due date, the GST to pay
/// (or refund due), what has been paid and what is still owing.
#[utoipa::path(get, path = "/{business_id}/gst/returns", tag = "gst", params(("business_id" = uuid::Uuid, Path)),
    responses((status = OK, body = GstReturns)))]
async fn list_returns(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<GstReturns>> {
    Ok(Json(gst_returns::returns(&state, &ctx).await?))
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BoxesQuery {
    /// The last day of the return's period.
    pub end: chrono::NaiveDate,
}

/// The figures for IRD's GST return (boxes 5 to 15) for one period, ready to copy into myIR.
#[utoipa::path(get, path = "/{business_id}/gst/returns/boxes", tag = "gst", params(("business_id" = uuid::Uuid, Path), BoxesQuery),
    responses((status = OK, body = GstReturnBoxes), (status = NOT_FOUND, body = crate::error::ErrorBody)))]
async fn return_boxes(
    State(state): State<AppState>,
    ctx: BusinessContext,
    super::extract::ApiQuery(q): super::extract::ApiQuery<BoxesQuery>,
) -> AppResult<Json<GstReturnBoxes>> {
    Ok(Json(gst_returns::boxes(&state, &ctx, q.end).await?))
}

/// Income tax for each income year: taxable income, tax by IRD's bands, tax withheld, residual
/// income tax, provisional tax (standard option) and terminal tax, with what has been paid.
#[utoipa::path(get, path = "/{business_id}/income-tax", tag = "gst", params(("business_id" = uuid::Uuid, Path)),
    responses((status = OK, body = IncomeTax)))]
async fn get_income_tax(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<IncomeTax>> {
    Ok(Json(income_tax::summary(&state, &ctx).await?))
}

/// Saves the owner's ACC work levy rate (dollars per $100 for their classification unit) and whether
/// they work full-time, for the ACC levy estimate on the Income tax page.
#[utoipa::path(put, path = "/{business_id}/income-tax/acc", tag = "gst", params(("business_id" = uuid::Uuid, Path)),
    request_body = AccSettings,
    responses((status = OK, body = AccSettings), (status = UNPROCESSABLE_ENTITY, body = crate::error::ErrorBody)))]
async fn save_acc_settings(
    State(state): State<AppState>,
    ctx: BusinessContext,
    super::extract::ApiJson(input): super::extract::ApiJson<AccSettings>,
) -> AppResult<Json<AccSettings>> {
    Ok(Json(
        income_tax::save_acc_settings(&state, &ctx, input).await?,
    ))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_returns))
        .routes(routes!(return_boxes))
        .routes(routes!(get_income_tax))
        .routes(routes!(save_acc_settings))
}
