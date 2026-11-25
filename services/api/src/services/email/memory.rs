//! In-memory provider for tests: records messages so tests can read links and attachments.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use super::{DeliveryReceipt, EmailAddress, EmailError, EmailMessage, EmailSender};

#[derive(Clone, Default)]
pub struct MemoryEmailSender {
    messages: Arc<Mutex<Vec<EmailMessage>>>,
}

impl MemoryEmailSender {
    pub fn messages(&self) -> Vec<EmailMessage> {
        self.messages.lock().map(|m| m.clone()).unwrap_or_default()
    }

    /// Waits (up to two seconds) for an email to `to` whose subject contains `subject_part`.
    /// Account emails are sent in the background, so tests must wait for them.
    pub async fn wait_for(&self, to: &str, subject_part: &str) -> Option<EmailMessage> {
        for _ in 0..100 {
            let found =
                self.messages().into_iter().rev().find(|m| {
                    m.to.email.eq_ignore_ascii_case(to) && m.subject.contains(subject_part)
                });
            if found.is_some() {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        None
    }
}

#[async_trait]
impl EmailSender for MemoryEmailSender {
    async fn send(
        &self,
        _from: &EmailAddress,
        message: EmailMessage,
    ) -> Result<DeliveryReceipt, EmailError> {
        if !message.to.email.contains('@') {
            return Err(EmailError::InvalidAddress(message.to.email));
        }
        let mut messages = self
            .messages
            .lock()
            .map_err(|_| EmailError::Delivery("mailbox poisoned".into()))?;
        messages.push(message);
        Ok(DeliveryReceipt {
            provider_reference: format!("memory-{}", messages.len()),
        })
    }
}
