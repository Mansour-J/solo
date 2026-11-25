//! Email delivery behind a provider abstraction. SMTP covers Mailpit (local), Amazon SES, Postmark,
//! Mailgun and most providers; `log` and `memory` exist for development and tests.

mod memory;
pub mod smtp;
pub mod templates;

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::{Brand, EmailConfig};

pub use memory::MemoryEmailSender;

#[derive(Debug, Clone)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub email: String,
}

impl EmailAddress {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name: (!name.trim().is_empty()).then_some(name),
            email: email.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub to: EmailAddress,
    pub reply_to: Option<EmailAddress>,
    pub subject: String,
    pub html: String,
    pub text: String,
    pub attachments: Vec<EmailAttachment>,
}

/// What the provider told us about a delivery attempt.
#[derive(Debug, Clone)]
pub struct DeliveryReceipt {
    /// Provider message ID or SMTP response, kept in the email history.
    pub provider_reference: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("invalid email address: {0}")]
    InvalidAddress(String),
    #[error("email could not be delivered: {0}")]
    Delivery(String),
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(
        &self,
        from: &EmailAddress,
        message: EmailMessage,
    ) -> Result<DeliveryReceipt, EmailError>;
}

/// The application's mailer: knows the sender identity and how to send in the background.
#[derive(Clone)]
pub struct Mailer {
    sender: Arc<dyn EmailSender>,
    from: EmailAddress,
}

impl Mailer {
    pub fn new(sender: Arc<dyn EmailSender>, brand: &Brand) -> Self {
        Self {
            sender,
            from: EmailAddress::new(
                brand.email_sender_name.clone(),
                brand.email_sender_address.clone(),
            ),
        }
    }

    /// A mailer sending as `from` (the owner's own mail account).
    pub fn with_from(sender: Arc<dyn EmailSender>, from: EmailAddress) -> Self {
        Self { sender, from }
    }

    pub fn from_config(
        config: &EmailConfig,
        brand: &Brand,
    ) -> anyhow::Result<(Self, Option<MemoryEmailSender>)> {
        let (sender, memory): (Arc<dyn EmailSender>, Option<MemoryEmailSender>) = match config {
            EmailConfig::Smtp(smtp) => (Arc::new(smtp::SmtpEmailSender::new(smtp)?), None),
            EmailConfig::Log => (Arc::new(LogEmailSender), None),
            EmailConfig::Memory => {
                let memory = MemoryEmailSender::default();
                (Arc::new(memory.clone()), Some(memory))
            }
        };
        Ok((Self::new(sender, brand), memory))
    }

    /// Sends and waits for the provider's answer (customer-facing documents, where status matters).
    pub async fn send(&self, message: EmailMessage) -> Result<DeliveryReceipt, EmailError> {
        self.sender.send(&self.from, message).await
    }

    /// Sends with a different display name (e.g. the business), keeping the configured sender address.
    pub async fn send_as(
        &self,
        display_name: &str,
        message: EmailMessage,
    ) -> Result<DeliveryReceipt, EmailError> {
        let from = EmailAddress::new(display_name.to_owned(), self.from.email.clone());
        self.sender.send(&from, message).await
    }

    /// Sends in the background. Used for account emails so that response time never reveals whether
    /// an email was sent (and therefore whether an account exists). Failures are logged.
    pub fn dispatch(&self, message: EmailMessage) {
        let mailer = self.clone();
        tokio::spawn(async move {
            let subject = message.subject.clone();
            if let Err(error) = mailer.send(message).await {
                tracing::error!(%error, subject, "account email failed");
            }
        });
    }
}

/// Development provider: writes the email to the log instead of sending it.
struct LogEmailSender;

#[async_trait]
impl EmailSender for LogEmailSender {
    async fn send(
        &self,
        _from: &EmailAddress,
        message: EmailMessage,
    ) -> Result<DeliveryReceipt, EmailError> {
        tracing::info!(to = %message.to.email, subject = %message.subject, body = %message.text, "email (log provider, not sent)");
        Ok(DeliveryReceipt {
            provider_reference: "logged".into(),
        })
    }
}
