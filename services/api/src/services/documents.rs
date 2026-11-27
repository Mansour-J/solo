//! Invoice and quote PDFs (Typst, rendered in-process) and sending them by email with a delivery log.

use std::sync::LazyLock;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use typst::foundations::{Bytes, Dict, Value};
use typst_as_lib::{TypstEngine, TypstTemplateMainFile};
use typst_layout::PagedDocument;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::Address;
use crate::domain::documents::{InvoiceDisplayStatus, QuoteDisplayStatus};
use crate::domain::money::Currency;
use crate::domain::tax::TaxTreatment;
use crate::domain::validation;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::businesses::{self, BusinessRow, SettingsRow};
use crate::repositories::documents::ItemRow;
use crate::repositories::invoices::{self as invoice_repo, NewEmailLog};
use crate::services::email::{EmailAddress, EmailAttachment, templates};
use crate::services::invoices::{self, Invoice, load_business};
use crate::services::quotes::{self, Quote};
use crate::services::storage::FileKind;
use crate::state::AppState;

static TEMPLATE: &str = include_str!("../../assets/templates/document.typ");
/// Inter, embedded for every PDF (documents and reports).
pub(crate) static FONTS: [&[u8]; 4] = [
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/Inter-Bold.ttf"),
];

static ENGINE: LazyLock<TypstEngine<TypstTemplateMainFile>> = LazyLock::new(|| {
    TypstEngine::builder()
        .main_file(TEMPLATE)
        .fonts(FONTS)
        .build()
});

#[derive(Debug, Serialize)]
struct Party {
    name: String,
    trading_name: String,
    business_name: String,
    contact_person: String,
    address_lines: Vec<String>,
    email: String,
    phone: String,
}

#[derive(Debug, Serialize)]
struct Meta {
    label: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct Line {
    description: String,
    quantity: String,
    unit_price: String,
    discount: String,
    gst: String,
    amount: String,
}

#[derive(Debug, Serialize)]
struct TotalRow {
    label: String,
    value: String,
    strong: bool,
}

#[derive(Debug, Serialize)]
struct Balance {
    label: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct PdfData {
    title: String,
    number: String,
    accent: String,
    watermark: String,
    footer_note: String,
    business: Party,
    customer: Party,
    customer_label: String,
    meta: Vec<Meta>,
    tax_lines: Vec<String>,
    price_label: String,
    lines: Vec<Line>,
    totals: Vec<TotalRow>,
    balance: Option<Balance>,
    notes: String,
    payment_lines: Vec<String>,
    footer: String,
}

/// Renders a PDF from prepared data on the blocking pool (Typst layout is CPU-bound).
async fn render(data: PdfData, logo: Option<Vec<u8>>) -> AppResult<Vec<u8>> {
    let json = serde_json::to_string(&data).map_err(AppError::internal)?;
    tokio::task::spawn_blocking(move || {
        let mut inputs = Dict::new();
        inputs.insert("data".into(), Value::Str(json.as_str().into()));
        if let Some(logo) = logo {
            inputs.insert("logo".into(), Value::Bytes(Bytes::new(logo)));
        }
        let document: PagedDocument = ENGINE
            .compile_with_input(inputs)
            .output
            .map_err(|e| anyhow::anyhow!("PDF layout failed: {e:?}"))?;
        typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
            .map_err(|e| anyhow::anyhow!("PDF export failed: {e:?}"))
    })
    .await
    .map_err(AppError::internal)?
    .map_err(AppError::Internal)
}

fn date(value: NaiveDate) -> String {
    value.format("%-d %b %Y").to_string()
}

fn formatted_gst_number(digits: &str) -> String {
    let padded = format!("{digits:0>9}");
    let groups = [&padded[0..3], &padded[3..6], &padded[6..9]];
    let first = if digits.len() == 8 {
        groups[0].trim_start_matches('0')
    } else {
        groups[0]
    };
    format!("{first}-{}-{}", groups[1], groups[2])
}

fn party(
    name: &str,
    trading: &str,
    business_name: &str,
    contact: &str,
    address: &Address,
    email: Option<&str>,
    phone: Option<&str>,
) -> Party {
    Party {
        name: name.to_owned(),
        trading_name: trading.to_owned(),
        business_name: business_name.to_owned(),
        contact_person: contact.to_owned(),
        address_lines: address.lines(),
        email: email.unwrap_or_default().to_owned(),
        phone: phone.unwrap_or_default().to_owned(),
    }
}

fn lines(items: &[ItemRow], currency: Currency) -> Vec<Line> {
    items
        .iter()
        .map(|i| Line {
            description: i.description.clone(),
            quantity: i.quantity.normalize().to_string(),
            unit_price: currency.format(i.unit_price),
            discount: if i.discount_percent.is_zero() {
                "—".into()
            } else {
                format!("{}%", i.discount_percent.normalize())
            },
            gst: match i.tax_treatment {
                TaxTreatment::Standard => format!(
                    "{}%",
                    (i.tax_rate * rust_decimal::Decimal::ONE_HUNDRED).normalize()
                ),
                TaxTreatment::ZeroRated => "0%".into(),
                TaxTreatment::Exempt => "Exempt".into(),
                TaxTreatment::NoGst => "—".into(),
            },
            amount: currency.format(i.net_amount),
        })
        .collect()
}

struct Common<'a> {
    business: &'a BusinessRow,
    settings: &'a SettingsRow,
    brand_colour: &'a str,
}

impl Common<'_> {
    fn accent(&self) -> String {
        if self.settings.invoice_accent_color.is_empty() {
            self.brand_colour.to_owned()
        } else {
            self.settings.invoice_accent_color.clone()
        }
    }

    fn business_party(&self) -> Party {
        let address: Address =
            serde_json::from_value(self.business.address.clone()).unwrap_or_default();
        party(
            &self.business.name,
            &self.business.trading_name,
            "",
            "",
            &address,
            self.business.email.as_deref(),
            self.business.phone.as_deref(),
        )
    }

    fn tax_lines(&self, registered: bool) -> Vec<String> {
        let mut lines = Vec::new();
        if registered && let Some(number) = &self.business.gst_number {
            lines.push(format!("GST number: {}", formatted_gst_number(number)));
        }
        if let Some(nzbn) = &self.business.nzbn {
            lines.push(format!("NZBN: {nzbn}"));
        }
        lines
    }
}

fn gst_rate_label(items: &[ItemRow]) -> String {
    items
        .iter()
        .find(|i| i.tax_treatment == TaxTreatment::Standard)
        .map(|i| {
            format!(
                "GST {}%",
                (i.tax_rate * rust_decimal::Decimal::ONE_HUNDRED).normalize()
            )
        })
        .unwrap_or_else(|| "GST".into())
}

fn invoice_data(c: &Common<'_>, invoice: &Invoice) -> PdfData {
    let currency = Currency::from_code(&invoice.currency).unwrap_or(Currency::all()[0]);
    let registered = invoice.gst_total > 0
        || invoice
            .lines
            .iter()
            .any(|l| l.tax_treatment != TaxTreatment::NoGst);
    let number = invoice.number.clone().unwrap_or_else(|| "Draft".into());
    let mut meta = vec![
        Meta {
            label: "Issue date".into(),
            value: date(invoice.issue_date),
        },
        Meta {
            label: "Due date".into(),
            value: date(invoice.due_date),
        },
    ];
    if !invoice.reference.is_empty() {
        meta.push(Meta {
            label: "Reference".into(),
            value: invoice.reference.clone(),
        });
    }
    let mut totals = vec![TotalRow {
        label: "Subtotal".into(),
        value: currency.format(invoice.subtotal),
        strong: false,
    }];
    if registered {
        totals.push(TotalRow {
            label: gst_rate_label(&invoice.lines),
            value: currency.format(invoice.gst_total),
            strong: false,
        });
    }
    totals.push(TotalRow {
        label: format!("Total {}", invoice.currency),
        value: currency.format(invoice.total),
        strong: true,
    });
    if invoice.paid > 0 {
        totals.push(TotalRow {
            label: "Paid".into(),
            value: format!("−{}", currency.format(invoice.paid)),
            strong: false,
        });
    }
    let mut payment_lines = Vec::new();
    if !c.settings.bank_account_name.is_empty() {
        payment_lines.push(format!("Account name: {}", c.settings.bank_account_name));
    }
    if !c.settings.bank_account_number.is_empty() {
        payment_lines.push(format!(
            "Account number: {}",
            c.settings.bank_account_number
        ));
    }
    if !payment_lines.is_empty() {
        payment_lines.push(format!("Reference: {number}"));
    }
    payment_lines.extend(
        c.settings
            .payment_instructions
            .lines()
            .map(str::to_owned)
            .filter(|l| !l.trim().is_empty()),
    );
    PdfData {
        title: if registered {
            "Tax invoice".into()
        } else {
            "Invoice".into()
        },
        accent: c.accent(),
        watermark: match invoice.display_status {
            InvoiceDisplayStatus::Draft => "DRAFT".into(),
            InvoiceDisplayStatus::Cancelled => "CANCELLED".into(),
            InvoiceDisplayStatus::Paid => "PAID".into(),
            _ => String::new(),
        },
        footer_note: format!("{} · {}", c.business.name, number),
        business: c.business_party(),
        customer: party(
            &invoice.customer.name,
            "",
            &invoice.customer.business_name,
            &invoice.customer.contact_person,
            &invoice.customer.billing_address,
            invoice.customer.email.as_deref(),
            None,
        ),
        customer_label: "Bill to".into(),
        meta,
        tax_lines: c.tax_lines(registered),
        price_label: if invoice.prices_include_gst {
            "Price (incl. GST)".into()
        } else {
            "Price".into()
        },
        lines: lines(&invoice.lines, currency),
        totals,
        balance: (invoice.status == "issued").then(|| Balance {
            label: if invoice.balance == 0 {
                "Paid in full".into()
            } else {
                "Amount due".into()
            },
            value: currency.format(invoice.balance),
        }),
        notes: invoice.notes.clone(),
        payment_lines: if invoice.status == "issued" && invoice.balance > 0 {
            payment_lines
        } else {
            Vec::new()
        },
        footer: invoice.footer.clone(),
        number,
    }
}

fn quote_data(c: &Common<'_>, quote: &Quote) -> PdfData {
    let currency = Currency::from_code(&quote.currency).unwrap_or(Currency::all()[0]);
    let registered = quote.gst_total > 0
        || quote
            .lines
            .iter()
            .any(|l| l.tax_treatment != TaxTreatment::NoGst);
    let mut meta = vec![
        Meta {
            label: "Date".into(),
            value: date(quote.issue_date),
        },
        Meta {
            label: "Valid until".into(),
            value: date(quote.expiry_date),
        },
    ];
    if !quote.reference.is_empty() {
        meta.push(Meta {
            label: "Reference".into(),
            value: quote.reference.clone(),
        });
    }
    let mut totals = vec![TotalRow {
        label: "Subtotal".into(),
        value: currency.format(quote.subtotal),
        strong: false,
    }];
    if registered {
        totals.push(TotalRow {
            label: gst_rate_label(&quote.lines),
            value: currency.format(quote.gst_total),
            strong: false,
        });
    }
    totals.push(TotalRow {
        label: format!("Total {}", quote.currency),
        value: currency.format(quote.total),
        strong: true,
    });
    PdfData {
        title: "Quote".into(),
        number: quote.number.clone(),
        accent: c.accent(),
        watermark: match quote.display_status {
            QuoteDisplayStatus::Expired => "EXPIRED".into(),
            QuoteDisplayStatus::Rejected => "DECLINED".into(),
            _ => String::new(),
        },
        footer_note: format!("{} · {}", c.business.name, quote.number),
        business: c.business_party(),
        customer: party(
            &quote.customer.name,
            "",
            &quote.customer.business_name,
            &quote.customer.contact_person,
            &quote.customer.billing_address,
            quote.customer.email.as_deref(),
            None,
        ),
        customer_label: "Prepared for".into(),
        meta,
        tax_lines: c.tax_lines(registered),
        price_label: if quote.prices_include_gst {
            "Price (incl. GST)".into()
        } else {
            "Price".into()
        },
        lines: lines(&quote.lines, currency),
        totals,
        balance: None,
        notes: quote.notes.clone(),
        payment_lines: Vec::new(),
        footer: quote.footer.clone(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentType {
    Invoice,
    Quote,
}

impl DocumentType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Invoice => "invoice",
            Self::Quote => "quote",
        }
    }
}

pub struct RenderedDocument {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub number: String,
    pub total: String,
    pub due: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: String,
    pub business_name: String,
    pub business_email: Option<String>,
}

async fn logo_bytes(state: &AppState, business: &BusinessRow) -> Option<Vec<u8>> {
    let key = business.logo_object_key.as_ref()?;
    // Typst embeds PNG and JPEG; WebP logos fall back to the business name.
    match FileKind::from_extension(key) {
        Some(FileKind::Png | FileKind::Jpeg) => state.storage.get(key).await.ok().flatten(),
        _ => None,
    }
}

pub async fn render_document(
    state: &AppState,
    ctx: &BusinessContext,
    kind: DocumentType,
    id: Uuid,
) -> AppResult<RenderedDocument> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let settings = businesses::settings(&mut tx, ctx.business_id).await?;
    let common = Common {
        business: &business,
        settings: &settings,
        brand_colour: &state.config.brand.primary_color,
    };
    let currency = Currency::from_code(&business.currency).unwrap_or(Currency::all()[0]);
    let (data, rendered) = match kind {
        DocumentType::Invoice => {
            let invoice = invoices::load_for_document(&mut tx, &business, id).await?;
            let data = invoice_data(&common, &invoice);
            let number = invoice.number.clone().unwrap_or_else(|| "draft".into());
            let meta = RenderedDocument {
                filename: format!("{}.pdf", sanitize(&format!("Invoice {number}"))),
                bytes: Vec::new(),
                total: currency.format(invoice.balance.max(0)),
                due: Some(date(invoice.due_date)),
                customer_email: invoice.customer.email.clone(),
                customer_name: invoice.customer.name.clone(),
                business_name: business.name.clone(),
                business_email: business.email.clone(),
                number,
            };
            (data, meta)
        }
        DocumentType::Quote => {
            let quote = quotes::load_for_document(&mut tx, &business, id).await?;
            let data = quote_data(&common, &quote);
            let meta = RenderedDocument {
                filename: format!("{}.pdf", sanitize(&format!("Quote {}", quote.number))),
                bytes: Vec::new(),
                total: currency.format(quote.total),
                due: Some(date(quote.expiry_date)),
                customer_email: quote.customer.email.clone(),
                customer_name: quote.customer.name.clone(),
                business_name: business.name.clone(),
                business_email: business.email.clone(),
                number: quote.number,
            };
            (data, meta)
        }
    };
    tx.commit().await?;
    let bytes = render(data, logo_bytes(state, &business).await).await?;
    Ok(RenderedDocument { bytes, ..rendered })
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ─── Email ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SendDocumentInput {
    /// Defaults to the customer's email address.
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    /// Personal message above the summary. Defaults to a short greeting.
    #[serde(default)]
    pub message: Option<String>,
}

pub async fn send_document(
    state: &AppState,
    ctx: &BusinessContext,
    kind: DocumentType,
    id: Uuid,
    input: SendDocumentInput,
) -> AppResult<()> {
    crate::services::businesses::refuse_demo_email(state, ctx).await?;
    let Some(mailer) = crate::services::email_settings::mailer(state, ctx.user_id()).await? else {
        return Err(AppError::Conflict(
            "Email isn’t set up yet: add your email account in Settings › Email, or download the PDF and send it yourself.".into(),
        ));
    };
    {
        let mut tx = state
            .db
            .begin_business(ctx.user_id(), ctx.business_id)
            .await?;
        let business = load_business(&mut tx, ctx.business_id).await?;
        match kind {
            DocumentType::Invoice => {
                let invoice = invoices::load_for_document(&mut tx, &business, id).await?;
                if invoice.status != "issued" {
                    return Err(AppError::Conflict(
                        "Issue the invoice before sending it.".into(),
                    ));
                }
            }
            DocumentType::Quote => {
                let quote = quotes::load_for_document(&mut tx, &business, id).await?;
                if quote.converted_invoice_id.is_some() {
                    return Err(AppError::Conflict(
                        "This quote was converted to an invoice.".into(),
                    ));
                }
            }
        }
        tx.commit().await?;
    }
    let doc = render_document(state, ctx, kind, id).await?;
    let mut errors = FieldErrors::new();
    let to = validation::normalize_email(
        input
            .to
            .as_deref()
            .or(doc.customer_email.as_deref())
            .unwrap_or_default(),
    );
    errors.check(
        validation::is_valid_email(&to),
        "to",
        "Enter the customer’s email address.",
    );
    let noun = match kind {
        DocumentType::Invoice => "Invoice",
        DocumentType::Quote => "Quote",
    };
    let subject = input
        .subject
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{noun} {} from {}", doc.number, doc.business_name));
    errors.check(
        subject.chars().count() <= 200,
        "subject",
        "Use 200 characters or fewer.",
    );
    let message = input
        .message
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| {
            format!(
                "Hi {},\n\nPlease find {} {} attached.",
                doc.customer_name,
                noun.to_lowercase(),
                doc.number
            )
        });
    errors.check(
        message.chars().count() <= 4000,
        "message",
        "Use 4000 characters or fewer.",
    );
    errors.into_result()?;

    let summary = match kind {
        DocumentType::Invoice => vec![
            format!("Amount due: {}", doc.total),
            format!("Due date: {}", doc.due.clone().unwrap_or_default()),
        ],
        DocumentType::Quote => vec![
            format!("Total: {}", doc.total),
            format!("Valid until: {}", doc.due.clone().unwrap_or_default()),
        ],
    };
    let mut email = templates::document(
        &state.config.brand,
        EmailAddress::new(doc.customer_name.clone(), to.clone()),
        &doc.business_name,
        subject.clone(),
        &message,
        &summary,
    );
    email.reply_to = doc
        .business_email
        .as_ref()
        .map(|e| EmailAddress::new(doc.business_name.clone(), e.clone()));
    email.attachments.push(EmailAttachment {
        filename: doc.filename.clone(),
        content_type: "application/pdf".into(),
        bytes: doc.bytes,
    });
    let result = mailer.send_as(&doc.business_name, email).await;

    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let (status, reference, error) = match &result {
        Ok(receipt) => ("sent", Some(receipt.provider_reference.clone()), None),
        Err(e) => ("failed", None, Some(e.to_string())),
    };
    invoice_repo::log_email(
        &mut tx,
        NewEmailLog {
            business_id: ctx.business_id,
            document_type: kind.as_str(),
            document_id: id,
            to_email: &to,
            subject: &subject,
            status,
            provider_reference: reference.as_deref(),
            error: error.as_deref(),
            sent_by: ctx.user_id(),
            reminder_day: None,
        },
    )
    .await?;
    if result.is_ok() {
        match kind {
            DocumentType::Invoice => invoices::mark_sent_in(&mut tx, id).await?,
            DocumentType::Quote => quotes::mark_sent_in(&mut tx, id).await?,
        }
    }
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: if result.is_ok() {
                "document.emailed"
            } else {
                "document.email_failed"
            },
            entity_type: kind.as_str(),
            entity_id: Some(id),
            summary: if result.is_ok() {
                format!("{noun} {} emailed to {to}", doc.number)
            } else {
                format!("{noun} {} could not be emailed to {to}", doc.number)
            },
            changes: serde_json::json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    result.map(|_| ()).map_err(|e| {
        AppError::ServiceUnavailable(format!(
            "The email couldn’t be sent: {e}. Nothing was marked as sent."
        ))
    })
}
