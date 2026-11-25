//! The owner's profile (name and preferences) and the full data export.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::Owner;
use crate::db::Db;
use crate::domain::user::{Preferences, SUPPORTED_LANGUAGES, SUPPORTED_NUMBER_LOCALES};
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::businesses;
use crate::repositories::users::{self, UserRow};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct UserProfile {
    pub id: Uuid,
    /// Empty until the owner enters a name.
    pub first_name: String,
    pub last_name: String,
    pub preferences: Preferences,
    pub created_at: DateTime<Utc>,
}

impl From<&UserRow> for UserProfile {
    fn from(user: &UserRow) -> Self {
        Self {
            id: user.id,
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            preferences: Preferences::from_json(&user.preferences),
            created_at: user.created_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProfileInput {
    pub first_name: String,
    pub last_name: String,
    pub preferences: Preferences,
}

/// The owner's user ID, creating the owner on first start.
pub async fn ensure_owner(db: &Db) -> anyhow::Result<Uuid> {
    let mut tx = db.begin().await?;
    // Two processes starting at once must not create two owners.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('solo.ensure_owner'))")
        .execute(&mut *tx)
        .await?;
    let owner = match users::first(&mut tx).await? {
        Some(user) => user.id,
        None => users::insert(&mut tx, Uuid::now_v7()).await?.id,
    };
    tx.commit().await?;
    Ok(owner)
}

pub fn full_name(user: &UserRow) -> String {
    format!("{} {}", user.first_name, user.last_name)
        .trim()
        .to_owned()
}

async fn load_user(state: &AppState, user_id: Uuid) -> AppResult<UserRow> {
    let mut conn = state.db.pool().acquire().await?;
    users::find_by_id(&mut conn, user_id)
        .await?
        .ok_or(AppError::NotFound("Profile"))
}

pub async fn profile(state: &AppState, owner: &Owner) -> AppResult<UserProfile> {
    Ok(UserProfile::from(&load_user(state, owner.user_id).await?))
}

pub async fn update_profile(
    state: &AppState,
    owner: &Owner,
    input: UpdateProfileInput,
) -> AppResult<UserProfile> {
    let mut errors = FieldErrors::new();
    let first_name = input.first_name.trim();
    let last_name = input.last_name.trim();
    errors.check(
        first_name.chars().count() <= 100,
        "first_name",
        "Use up to 100 characters.",
    );
    errors.check(
        last_name.chars().count() <= 100,
        "last_name",
        "Use up to 100 characters.",
    );
    errors.check(
        SUPPORTED_LANGUAGES.contains(&input.preferences.language.as_str()),
        "preferences.language",
        "Choose a supported language.",
    );
    errors.check(
        SUPPORTED_NUMBER_LOCALES.contains(&input.preferences.number_locale.as_str()),
        "preferences.number_locale",
        "Choose a supported number format.",
    );
    errors.into_result()?;
    let preferences = serde_json::to_value(&input.preferences).map_err(AppError::internal)?;
    let mut tx = state.db.begin_user(owner.user_id).await?;
    let before = users::find_by_id(&mut tx, owner.user_id)
        .await?
        .ok_or(AppError::NotFound("Profile"))?;
    let user =
        users::update_profile(&mut tx, owner.user_id, first_name, last_name, &preferences).await?;
    if before.first_name != user.first_name || before.last_name != user.last_name {
        audit::record(
            &mut tx,
            &owner.actor(),
            audit::Entry {
                business_id: None,
                action: "account.profile_updated",
                entity_type: "user",
                entity_id: Some(user.id),
                summary: "Name updated".into(),
                changes: serde_json::json!({ "name": { "from": full_name(&before), "to": full_name(&user) } }),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(UserProfile::from(&user))
}

/// A machine-readable copy of the profile and every business.
pub async fn export(state: &AppState, owner: &Owner) -> AppResult<serde_json::Value> {
    let user = load_user(state, owner.user_id).await?;
    let memberships = {
        let mut tx = state.db.begin_user(owner.user_id).await?;
        let rows = businesses::list_for_user(&mut tx, owner.user_id).await?;
        tx.commit().await?;
        rows
    };
    let mut business_exports = Vec::new();
    for membership in memberships {
        business_exports.push(
            crate::services::businesses::export_business(
                state,
                owner.user_id,
                membership.business_id,
            )
            .await?,
        );
    }
    Ok(serde_json::json!({
        "format": "account-export",
        "format_version": 2,
        "exported_at": Utc::now(),
        "product": state.config.brand.name,
        "user": UserProfile::from(&user),
        "businesses": business_exports,
    }))
}
