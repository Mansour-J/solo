//! SMTP provider (lettre). Works with Mailpit locally and with any SMTP relay in production.

use async_trait::async_trait;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use super::{DeliveryReceipt, EmailAddress, EmailError, EmailMessage, EmailSender};
use crate::config::{SmtpConfig, SmtpTls};

pub struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpEmailSender {
    pub fn new(config: &SmtpConfig) -> anyhow::Result<Self> {
        let mut builder = match config.tls {
            SmtpTls::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
            SmtpTls::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)?
            }
            SmtpTls::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)?,
        }
        .port(config.port);
        if let (Some(user), Some(pass)) = (&config.username, &config.password) {
            builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }
        Ok(Self {
            transport: builder.build(),
        })
    }
}

fn mailbox(address: &EmailAddress) -> Result<Mailbox, EmailError> {
    let email = address
        .email
        .parse()
        .map_err(|_| EmailError::InvalidAddress(address.email.clone()))?;
    Ok(Mailbox::new(address.name.clone(), email))
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(
        &self,
        from: &EmailAddress,
        message: EmailMessage,
    ) -> Result<DeliveryReceipt, EmailError> {
        let mut builder = Message::builder()
            .from(mailbox(from)?)
            .to(mailbox(&message.to)?)
            .subject(message.subject);
        if let Some(reply_to) = &message.reply_to {
            builder = builder.reply_to(mailbox(reply_to)?);
        }
        let body = MultiPart::alternative_plain_html(message.text, message.html);
        let email = if message.attachments.is_empty() {
            builder.multipart(body)
        } else {
            let mut mixed = MultiPart::mixed().multipart(body);
            for attachment in message.attachments {
                let content_type =
                    ContentType::parse(&attachment.content_type).unwrap_or(ContentType::TEXT_PLAIN);
                let part: SinglePart =
                    Attachment::new(attachment.filename).body(attachment.bytes, content_type);
                mixed = mixed.singlepart(part);
            }
            builder.multipart(mixed)
        }
        .map_err(|e| EmailError::Delivery(e.to_string()))?;

        let response = self
            .transport
            .send(email)
            .await
            .map_err(|e| EmailError::Delivery(e.to_string()))?;
        Ok(DeliveryReceipt {
            provider_reference: response.message().collect::<Vec<_>>().join(" "),
        })
    }
}
