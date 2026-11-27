//! The weekly notices email (SL-606): once a week, when the app runs, the notices the owner hasn't
//! dismissed in each business, in one email. Off until the owner turns it on in Settings › Email.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::context::{BusinessContext, Owner};
use crate::domain::validation;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::services::email::{EmailAddress, EmailMessage, templates};
use crate::services::{businesses, email_settings, insights};
use crate::state::AppState;

/// How often the email goes.
const EVERY_DAYS: i64 = 7;

#[derive(Debug, Serialize, ToSchema)]
pub struct NoticesEmail {
    pub enabled: bool,
    /// Where it goes; `null` uses the send-from address in the email settings.
    pub to: Option<String>,
    /// Where it goes when `to` is empty, if email is set up.
    pub default_to: Option<String>,
    pub last_sent_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NoticesEmailInput {
    pub enabled: bool,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NoticesRunInput {
    /// Send now even if one went out this week (the "Send now" button).
    #[serde(default)]
    pub now: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NoticesRun {
    /// An email went out.
    pub sent: bool,
    /// How many notices it listed.
    pub notices: usize,
    /// Why nothing went, when the owner asked to send now.
    pub reason: Option<String>,
}

#[derive(sqlx::FromRow)]
struct Row {
    notices_email: bool,
    notices_email_to: Option<String>,
    notices_emailed_at: Option<DateTime<Utc>>,
}

async fn row(state: &AppState, owner: &Owner) -> AppResult<Row> {
    let mut tx = state.db.begin_user(owner.user_id).await?;
    let r = sqlx::query_as(
        "SELECT notices_email, notices_email_to, notices_emailed_at FROM users WHERE id = $1",
    )
    .bind(owner.user_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(r)
}

pub async fn get(state: &AppState, owner: &Owner) -> AppResult<NoticesEmail> {
    let r = row(state, owner).await?;
    let settings = email_settings::get(state, owner).await?;
    Ok(NoticesEmail {
        enabled: r.notices_email,
        to: r.notices_email_to,
        default_to: (settings.configured && !settings.from_address.is_empty())
            .then_some(settings.from_address),
        last_sent_at: r.notices_emailed_at,
    })
}

pub async fn save(
    state: &AppState,
    owner: &Owner,
    input: NoticesEmailInput,
) -> AppResult<NoticesEmail> {
    let to = input
        .to
        .map(|t| validation::normalize_email(&t))
        .filter(|t| !t.is_empty());
    let mut errors = FieldErrors::new();
    if let Some(t) = &to {
        errors.check(
            validation::is_valid_email(t),
            "to",
            "Enter an email address, or leave it empty.",
        );
    }
    errors.into_result()?;
    let mut tx = state.db.begin_user(owner.user_id).await?;
    sqlx::query("UPDATE users SET notices_email = $2, notices_email_to = $3 WHERE id = $1")
        .bind(owner.user_id)
        .bind(input.enabled)
        .bind(&to)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    get(state, owner).await
}

async fn mark_sent(state: &AppState, owner: &Owner) -> AppResult<()> {
    let mut tx = state.db.begin_user(owner.user_id).await?;
    sqlx::query("UPDATE users SET notices_emailed_at = now() WHERE id = $1")
        .bind(owner.user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

fn skipped(reason: Option<&str>) -> NoticesRun {
    NoticesRun {
        sent: false,
        notices: 0,
        reason: reason.map(Into::into),
    }
}

/// Sends the email when it's on and a week has passed (or `now`). The app calls this when it opens.
pub async fn run(state: &AppState, owner: &Owner, input: NoticesRunInput) -> AppResult<NoticesRun> {
    let r = row(state, owner).await?;
    if !input.now {
        if !r.notices_email {
            return Ok(skipped(None));
        }
        if r.notices_emailed_at
            .is_some_and(|at| Utc::now() - at < Duration::days(EVERY_DAYS))
        {
            return Ok(skipped(None));
        }
    }
    let Some(mailer) = email_settings::mailer(state, owner.user_id).await? else {
        return Ok(skipped(Some(
            "Email isn’t set up: add your email account first.",
        )));
    };
    let settings = email_settings::get(state, owner).await?;
    let to = r
        .notices_email_to
        .clone()
        .or_else(|| (!settings.from_address.is_empty()).then(|| settings.from_address.clone()));
    let Some(to) = to else {
        return Ok(skipped(Some("Enter the address to send it to.")));
    };

    let mut paragraphs = Vec::new();
    let mut count = 0;
    let list = businesses::list(state, owner).await?;
    let several = list.iter().filter(|b| !b.is_demo).count() > 1;
    // The demo business's notices are about made-up records.
    for b in list.iter().filter(|b| !b.is_demo) {
        let ctx = BusinessContext {
            owner: owner.clone(),
            business_id: b.id,
        };
        let found = insights::build(state, &ctx).await?;
        for n in &found.notices {
            count += 1;
            paragraphs.push(if several {
                format!("{}: {}", b.name, n.message)
            } else {
                n.message.clone()
            });
        }
    }
    if count == 0 {
        // Nothing to say this week: wait another week before looking again.
        mark_sent(state, owner).await?;
        return Ok(skipped(Some(
            "There are no notices right now, so there was nothing to send.",
        )));
    }
    let brand = &state.config.brand;
    paragraphs.insert(
        0,
        format!(
            "{count} {} you haven’t dismissed:",
            if count == 1 { "notice" } else { "notices" }
        ),
    );
    let (html, text) = templates::render(
        brand,
        &templates::Content {
            heading: "Your notices this week".into(),
            paragraphs,
            button: None,
            note: Some(format!(
                "Open {} to see the details or dismiss them. To stop these emails, turn off the weekly notices email in Settings › Email.",
                brand.name
            )),
        },
        None,
    );
    let message = EmailMessage {
        to: EmailAddress::new(String::new(), to),
        subject: format!(
            "{}: {count} {} this week",
            brand.name,
            if count == 1 { "notice" } else { "notices" }
        ),
        html,
        text,
        reply_to: None,
        attachments: Vec::new(),
    };
    mailer.send(message).await.map_err(|e| {
        AppError::Conflict(format!(
            "The notices email couldn’t be sent: {e}. Check Settings › Email."
        ))
    })?;
    mark_sent(state, owner).await?;
    Ok(NoticesRun {
        sent: true,
        notices: count,
        reason: None,
    })
}
