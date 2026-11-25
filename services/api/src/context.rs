//! Who a request acts for. Solo is for one person, so there is no sign-in: every request acts as the
//! owner, whose ID the app attaches to each request ([`OwnerId`], added by `api::app`).
//!
//! Business data is still read inside `Db::begin_business(owner, business)`, so row-level security
//! scopes every query to one business.

use axum::extract::{FromRequestParts, RawPathParams};
use axum::http::request::Parts;
use uuid::Uuid;

use crate::error::AppError;
use crate::repositories::{audit::Actor, businesses};
use crate::state::AppState;

/// The owner's user ID, attached to every request as an extension.
#[derive(Debug, Clone, Copy)]
pub struct OwnerId(pub Uuid);

/// A request, acting as the owner.
#[derive(Debug, Clone)]
pub struct Owner {
    pub user_id: Uuid,
    pub ip: Option<String>,
    pub request_id: Option<String>,
}

impl Owner {
    pub fn actor(&self) -> Actor {
        Actor {
            user_id: Some(self.user_id),
            ip_address: self.ip.clone(),
            request_id: self.request_id.clone(),
        }
    }
}

impl FromRequestParts<AppState> for Owner {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let OwnerId(user_id) = *parts.extensions.get::<OwnerId>().ok_or_else(|| {
            AppError::internal(anyhow::anyhow!("the router has no owner attached"))
        })?;
        let meta = crate::middleware::RequestMeta::from_request_parts(parts, state).await?;
        Ok(Self {
            user_id,
            ip: meta.ip,
            request_id: meta.request_id,
        })
    }
}

/// A request inside one business (from the `{business_id}` path segment). Extraction fails with 404
/// for a business that doesn't exist (or belongs to another owner), so handlers never see a stray ID.
#[derive(Debug, Clone)]
pub struct BusinessContext {
    pub owner: Owner,
    pub business_id: Uuid,
}

impl BusinessContext {
    pub fn user_id(&self) -> Uuid {
        self.owner.user_id
    }

    pub fn actor(&self) -> Actor {
        self.owner.actor()
    }
}

impl FromRequestParts<AppState> for BusinessContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let owner = Owner::from_request_parts(parts, state).await?;
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::NotFound("Business"))?;
        let business_id = params
            .iter()
            .find(|(name, _)| *name == "business_id")
            .and_then(|(_, value)| Uuid::parse_str(value).ok())
            .ok_or(AppError::NotFound("Business"))?;
        let mut tx = state.db.begin_user(owner.user_id).await?;
        let member = businesses::is_member(&mut tx, business_id, owner.user_id).await?;
        tx.commit().await?;
        if !member {
            return Err(AppError::NotFound("Business"));
        }
        Ok(Self { owner, business_id })
    }
}
