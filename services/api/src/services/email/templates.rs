//! Branded email content. Each template produces a subject, an HTML body and a plain-text body.
//! User-supplied values are HTML-escaped. Brand values come from configuration, never literals.

use super::{EmailAddress, EmailMessage};
use crate::config::Brand;

pub fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Building blocks of a transactional email.
pub struct Content<'a> {
    pub heading: String,
    pub paragraphs: Vec<String>,
    pub button: Option<(&'a str, String)>,
    /// Small print under the button (expiry, "ignore if this wasn't you").
    pub note: Option<String>,
}

/// Renders content into the shared layout. `accent` overrides the brand colour (business branding).
pub fn render(brand: &Brand, content: &Content<'_>, accent: Option<&str>) -> (String, String) {
    let colour = accent
        .filter(|c| crate::domain::validation::is_hex_colour(c))
        .unwrap_or(&brand.primary_color);
    let mut html = String::new();
    html.push_str(&format!(
        "<!doctype html><html><body style=\"margin:0;background:#f4f5f7;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1f2937\">\
         <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"padding:32px 12px\"><tr><td align=\"center\">\
         <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"max-width:560px;background:#ffffff;border-radius:12px;overflow:hidden;border:1px solid #e5e7eb\">\
         <tr><td style=\"background:{colour};padding:18px 28px;color:#ffffff;font-size:18px;font-weight:600\">{name}</td></tr>\
         <tr><td style=\"padding:28px\"><h1 style=\"margin:0 0 16px;font-size:20px;line-height:1.3\">{heading}</h1>",
        name = escape_html(&brand.name),
        heading = escape_html(&content.heading),
    ));
    for paragraph in &content.paragraphs {
        html.push_str(&format!(
            "<p style=\"margin:0 0 14px;font-size:15px;line-height:1.55\">{}</p>",
            escape_html(paragraph)
        ));
    }
    if let Some((label, url)) = &content.button {
        html.push_str(&format!(
            "<p style=\"margin:22px 0\"><a href=\"{url}\" style=\"display:inline-block;background:{colour};color:#ffffff;text-decoration:none;\
             padding:11px 20px;border-radius:8px;font-weight:600;font-size:15px\">{label}</a></p>\
             <p style=\"margin:0 0 14px;font-size:13px;color:#6b7280;word-break:break-all\">Or paste this link into your browser: {url}</p>",
            url = escape_html(url),
            label = escape_html(label),
        ));
    }
    if let Some(note) = &content.note {
        html.push_str(&format!(
            "<p style=\"margin:18px 0 0;font-size:13px;color:#6b7280\">{}</p>",
            escape_html(note)
        ));
    }
    html.push_str(&format!(
        "</td></tr><tr><td style=\"padding:16px 28px;border-top:1px solid #e5e7eb;font-size:12px;color:#9ca3af\">\
         Need help? Contact {support}. {copyright}</td></tr></table></td></tr></table></body></html>",
        support = escape_html(&brand.support_email),
        copyright = escape_html(&brand.copyright),
    ));

    let mut text = format!("{}\n\n", content.heading);
    for paragraph in &content.paragraphs {
        text.push_str(paragraph);
        text.push_str("\n\n");
    }
    if let Some((label, url)) = &content.button {
        text.push_str(&format!("{label}: {url}\n\n"));
    }
    if let Some(note) = &content.note {
        text.push_str(note);
        text.push_str("\n\n");
    }
    text.push_str(&format!(
        "— {}\nNeed help? Contact {}.\n",
        brand.name, brand.support_email
    ));
    (html, text)
}

fn message(to: EmailAddress, subject: String, (html, text): (String, String)) -> EmailMessage {
    EmailMessage {
        to,
        reply_to: None,
        subject,
        html,
        text,
        attachments: Vec::new(),
    }
}

/// A customer-facing document email (invoice or quote) sent on behalf of a business.
pub fn document(
    brand: &Brand,
    to: EmailAddress,
    business_name: &str,
    subject: String,
    body: &str,
    summary: &[String],
) -> EmailMessage {
    let mut paragraphs: Vec<String> = body
        .split("\n\n")
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    paragraphs.extend(summary.iter().cloned());
    let content = Content {
        heading: subject.clone(),
        paragraphs,
        button: None,
        note: Some(format!(
            "The PDF is attached. Reply to this email to contact {business_name}."
        )),
    };
    message(to, subject, render(brand, &content, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brand() -> Brand {
        Brand::load(|_| None).expect("brand")
    }

    #[test]
    fn user_values_are_escaped() {
        let msg = document(
            &brand(),
            EmailAddress::new("x", "x@y.nz"),
            "Tui & Co",
            "Invoice INV-1".into(),
            "Hi <script>\n\nThanks",
            &["Total: $10 & GST".into()],
        );
        assert!(!msg.html.contains("<script>"));
        assert!(msg.html.contains("&lt;script&gt;"));
        assert!(msg.html.contains("$10 &amp; GST"));
        assert!(msg.text.contains("Hi <script>"));
    }

    #[test]
    fn templates_use_the_configured_brand() {
        let renamed = Brand::load(|k| (k == "APP_NAME").then(|| "Ledgerly".into())).expect("brand");
        let msg = document(
            &renamed,
            EmailAddress::new("", "x@y.nz"),
            "Tui",
            "Quote Q-1".into(),
            "Hello",
            &[],
        );
        assert_eq!(msg.subject, "Quote Q-1");
        assert!(msg.html.contains("Ledgerly"));
    }
}
