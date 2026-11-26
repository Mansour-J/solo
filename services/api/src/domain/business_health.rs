//! Business health (Settings › Business health): whether the business is set up to keep correct
//! books, issue valid tax invoices and get paid. Each rule reads plain facts about the business and
//! says what it found, why it matters and where to fix it. Rules that don't apply to this business
//! (GST checks for a business that isn't registered) say so instead of passing or failing.
//!
//! Whether the app itself works (database, storage, email server) is System status
//! (`services/system_status.rs`); what the figures mean is Insights.

use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Something is wrong that affects tax or what customers receive: fix it now.
    Critical,
    /// Likely to cause a problem later.
    Warning,
    /// Worth doing, nothing breaks without it.
    Recommendation,
    Passed,
    /// Doesn't apply to this business.
    NotApplicable,
}

/// The part of the business a check belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthArea {
    /// Business details on invoices and documents.
    Details,
    Gst,
    /// Invoicing and getting paid.
    GettingPaid,
    /// Records IRD expects you to keep.
    Records,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct HealthCheck {
    /// Stable identifier, e.g. `gst-number`.
    pub key: &'static str,
    pub area: HealthArea,
    pub title: &'static str,
    pub status: HealthStatus,
    /// What the check found, in plain language.
    pub finding: String,
    /// Why it matters. Shown with problems.
    pub why: &'static str,
    /// What to do, when there's something to do.
    pub action: Option<&'static str>,
    /// The screen that fixes it.
    pub link: Option<String>,
}

/// What the rules need to know about a business.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub has_address: bool,
    pub has_email: bool,
    pub has_phone: bool,
    pub gst_registered: bool,
    pub has_gst_number: bool,
    pub has_gst_registered_from: bool,
    /// Not registered and sales over the last 12 months passed the GST threshold.
    pub over_gst_threshold: bool,
    /// Bank account number or payment instructions for invoices.
    pub has_payment_details: bool,
    /// Email can be sent (the owner's mail settings are on, or the server has a mail server).
    pub email_ready: bool,
    pub reminders_enabled: bool,
    /// Customers who owe money on an issued invoice.
    pub customers_owing: i64,
    /// …of whom have no email address.
    pub customers_owing_without_email: i64,
    /// Business expenses this financial year without a receipt.
    pub expenses_without_receipt: i64,
    /// …of which claim GST on more than $50 (IRD needs the supplier's tax invoice for those).
    pub gst_claims_without_receipt: i64,
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn check(
    key: &'static str,
    area: HealthArea,
    title: &'static str,
    why: &'static str,
) -> impl FnOnce(HealthStatus, String, Option<&'static str>, Option<&str>) -> HealthCheck {
    move |status, finding, action, link| HealthCheck {
        key,
        area,
        title,
        status,
        finding,
        why,
        // A passed or inapplicable check has nothing to do.
        action: action.filter(|_| status < HealthStatus::Passed),
        link: link
            .filter(|_| status < HealthStatus::Passed)
            .map(Into::into),
    }
}

const BUSINESS: &str = "/settings/business";
const INVOICING: &str = "/settings/invoicing";

pub fn checks(f: &Facts) -> Vec<HealthCheck> {
    use HealthStatus::*;
    let mut out = Vec::new();

    // ─── GST ────────────────────────────────────────────────────────────────
    let c = check(
        "gst-threshold",
        HealthArea::Gst,
        "GST registration",
        "IRD requires you to register for GST within 21 days of your sales passing $60,000 in 12 months, and you owe GST from the date you should have registered.",
    );
    out.push(if f.gst_registered {
        c(
            NotApplicable,
            "You’re registered for GST.".into(),
            None,
            None,
        )
    } else if f.over_gst_threshold {
        c(
            Critical,
            "Your sales over the last 12 months were over $60,000 and you aren’t registered for GST. Register with IRD, then turn on GST in your business details.".into(),
            Some("Turn on GST"),
            Some(BUSINESS),
        )
    } else {
        c(
            Passed,
            "Your sales over the last 12 months are under the $60,000 GST threshold.".into(),
            None,
            None,
        )
    });

    let c = check(
        "gst-number",
        HealthArea::Gst,
        "GST number",
        "A tax invoice must show your GST number. Without it, GST-registered customers can’t claim the GST on your invoices.",
    );
    out.push(match (f.gst_registered, f.has_gst_number) {
        (false, _) => c(
            NotApplicable,
            "You aren’t registered for GST.".into(),
            None,
            None,
        ),
        (true, false) => c(
            Critical,
            "You’re registered for GST but your GST number isn’t in your business details, so it isn’t on your invoices.".into(),
            Some("Add your GST number"),
            Some(BUSINESS),
        ),
        (true, true) => c(
            Passed,
            "Your GST number is on your invoices.".into(),
            None,
            None,
        ),
    });

    let c = check(
        "gst-registered-from",
        HealthArea::Gst,
        "GST registration date",
        "GST is claimed only on expenses from the date you registered. Without the date, GST is worked out on every expense, including ones from before you registered.",
    );
    out.push(match (f.gst_registered, f.has_gst_registered_from) {
        (false, _) => c(
            NotApplicable,
            "You aren’t registered for GST.".into(),
            None,
            None,
        ),
        (true, false) => c(
            Warning,
            "Your business details don’t say when you registered for GST.".into(),
            Some("Add the registration date"),
            Some(BUSINESS),
        ),
        (true, true) => c(
            Passed,
            "Your GST registration date is set.".into(),
            None,
            None,
        ),
    });

    // ─── Business details ───────────────────────────────────────────────────
    let c = check(
        "address",
        HealthArea::Details,
        "Business address",
        "Your address goes on invoices, quotes and statements. Customers and their accountants expect it there.",
    );
    out.push(if f.has_address {
        c(
            Passed,
            "Your address is on your invoices.".into(),
            None,
            None,
        )
    } else {
        c(
            Recommendation,
            "Your business details have no address, so your invoices don’t show one.".into(),
            Some("Add your address"),
            Some(BUSINESS),
        )
    });

    let c = check(
        "contact",
        HealthArea::Details,
        "Contact details",
        "Customers use the email address or phone number on your invoices to ask about them.",
    );
    out.push(if f.has_email || f.has_phone {
        c(
            Passed,
            "Your invoices show how to contact you.".into(),
            None,
            None,
        )
    } else {
        c(
            Recommendation,
            "Your business details have no email address or phone number.".into(),
            Some("Add contact details"),
            Some(BUSINESS),
        )
    });

    // ─── Getting paid ───────────────────────────────────────────────────────
    let c = check(
        "payment-details",
        HealthArea::GettingPaid,
        "How to pay you",
        "Invoices without a bank account or payment instructions leave customers asking how to pay, which delays payment.",
    );
    out.push(if f.has_payment_details {
        c(
            Passed,
            "Your invoices say how to pay you.".into(),
            None,
            None,
        )
    } else {
        c(
            Warning,
            "Your invoices have no bank account or payment instructions.".into(),
            Some("Add payment details"),
            Some(INVOICING),
        )
    });

    let c = check(
        "email",
        HealthArea::GettingPaid,
        "Sending email",
        "With email set up, invoices, quotes, statements and payment reminders are sent from the app.",
    );
    out.push(match (f.email_ready, f.reminders_enabled) {
        (true, _) => c(
            Passed,
            "Email is set up, so invoices and reminders can be sent from the app.".into(),
            None,
            None,
        ),
        (false, true) => c(
            Warning,
            "Payment reminders are on but email isn’t set up, so no reminders are being sent."
                .into(),
            Some("Set up email"),
            Some("/settings/email"),
        ),
        (false, false) => c(
            Recommendation,
            "Email isn’t set up, so you send invoices yourself.".into(),
            Some("Set up email"),
            Some("/settings/email"),
        ),
    });

    let c = check(
        "customer-emails",
        HealthArea::GettingPaid,
        "Customers who owe you",
        "Invoices, statements and reminders can only be emailed to customers with an email address.",
    );
    out.push(if f.customers_owing == 0 {
        c(
            NotApplicable,
            "No customer owes you money.".into(),
            None,
            None,
        )
    } else if f.customers_owing_without_email == 0 {
        c(
            Passed,
            "Every customer who owes you has an email address.".into(),
            None,
            None,
        )
    } else {
        c(
            Recommendation,
            format!(
                "{} who {} you {} no email address.",
                plural(f.customers_owing_without_email, "customer", "customers"),
                if f.customers_owing_without_email == 1 {
                    "owes"
                } else {
                    "owe"
                },
                if f.customers_owing_without_email == 1 {
                    "has"
                } else {
                    "have"
                },
            ),
            Some("Add email addresses"),
            Some("/customers"),
        )
    });

    // ─── Records ────────────────────────────────────────────────────────────
    let c = check(
        "receipts",
        HealthArea::Records,
        "Receipts",
        "IRD expects you to keep records of expenses for seven years, and a tax invoice from the supplier for each GST claim over $50.",
    );
    out.push(if f.expenses_without_receipt == 0 {
        c(
            Passed,
            "Every expense this financial year has a receipt.".into(),
            None,
            None,
        )
    } else {
        let missing = plural(f.expenses_without_receipt, "expense", "expenses");
        if f.gst_claims_without_receipt > 0 {
            c(
                Warning,
                format!(
                    "{missing} this financial year {} no receipt, including {}.",
                    if f.expenses_without_receipt == 1 {
                        "has"
                    } else {
                        "have"
                    },
                    if f.gst_claims_without_receipt == 1 {
                        "one that claims GST over $50".to_owned()
                    } else {
                        format!("{} that claim GST over $50", f.gst_claims_without_receipt)
                    },
                ),
                Some("Attach receipts"),
                Some("/expenses?receipt=missing"),
            )
        } else {
            c(
                Recommendation,
                format!(
                    "{missing} this financial year {} no receipt.",
                    if f.expenses_without_receipt == 1 {
                        "has"
                    } else {
                        "have"
                    },
                ),
                Some("Attach receipts"),
                Some("/expenses?receipt=missing"),
            )
        }
    });

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A business with everything in place.
    fn healthy() -> Facts {
        Facts {
            has_address: true,
            has_email: true,
            has_phone: true,
            gst_registered: true,
            has_gst_number: true,
            has_gst_registered_from: true,
            over_gst_threshold: false,
            has_payment_details: true,
            email_ready: true,
            reminders_enabled: true,
            customers_owing: 2,
            customers_owing_without_email: 0,
            expenses_without_receipt: 0,
            gst_claims_without_receipt: 0,
        }
    }

    fn find(checks: &[HealthCheck], key: &str) -> HealthCheck {
        checks
            .iter()
            .find(|c| c.key == key)
            .cloned()
            .expect("check")
    }

    fn status(f: &Facts, key: &str) -> HealthStatus {
        find(&checks(f), key).status
    }

    #[test]
    fn a_healthy_business_passes_everything_that_applies() {
        let all = checks(&healthy());
        assert_eq!(all.len(), 9);
        for c in &all {
            let expected = if c.key == "gst-threshold" {
                HealthStatus::NotApplicable
            } else {
                HealthStatus::Passed
            };
            assert_eq!(c.status, expected, "{}", c.key);
            assert!(c.action.is_none() && c.link.is_none(), "{}", c.key);
        }
    }

    #[test]
    fn gst_checks_follow_registration() {
        let missing = Facts {
            has_gst_number: false,
            has_gst_registered_from: false,
            ..healthy()
        };
        let all = checks(&missing);
        let number = find(&all, "gst-number");
        assert_eq!(number.status, HealthStatus::Critical);
        assert_eq!(number.link.as_deref(), Some("/settings/business"));
        assert!(number.action.is_some());
        assert_eq!(
            status(&missing, "gst-registered-from"),
            HealthStatus::Warning
        );

        let unregistered = Facts {
            gst_registered: false,
            has_gst_number: false,
            ..healthy()
        };
        assert_eq!(
            status(&unregistered, "gst-number"),
            HealthStatus::NotApplicable
        );
        assert_eq!(
            status(&unregistered, "gst-registered-from"),
            HealthStatus::NotApplicable
        );
        assert_eq!(status(&unregistered, "gst-threshold"), HealthStatus::Passed);
        let over = Facts {
            over_gst_threshold: true,
            ..unregistered
        };
        assert_eq!(status(&over, "gst-threshold"), HealthStatus::Critical);
    }

    #[test]
    fn details_on_invoices() {
        let bare = Facts {
            has_address: false,
            has_email: false,
            has_phone: false,
            has_payment_details: false,
            ..healthy()
        };
        assert_eq!(status(&bare, "address"), HealthStatus::Recommendation);
        assert_eq!(status(&bare, "contact"), HealthStatus::Recommendation);
        let pay = find(&checks(&bare), "payment-details");
        assert_eq!(pay.status, HealthStatus::Warning);
        assert_eq!(pay.link.as_deref(), Some("/settings/invoicing"));
        let phone_only = Facts {
            has_phone: true,
            ..bare
        };
        assert_eq!(status(&phone_only, "contact"), HealthStatus::Passed);
    }

    #[test]
    fn email_matters_more_when_reminders_are_on() {
        let off = Facts {
            email_ready: false,
            ..healthy()
        };
        let warning = find(&checks(&off), "email");
        assert_eq!(warning.status, HealthStatus::Warning);
        assert!(warning.finding.contains("reminders are on"));
        let no_reminders = Facts {
            reminders_enabled: false,
            ..off
        };
        assert_eq!(status(&no_reminders, "email"), HealthStatus::Recommendation);
    }

    #[test]
    fn customers_who_owe_without_an_email_address() {
        let none_owing = Facts {
            customers_owing: 0,
            ..healthy()
        };
        assert_eq!(
            status(&none_owing, "customer-emails"),
            HealthStatus::NotApplicable
        );
        let one = Facts {
            customers_owing_without_email: 1,
            ..healthy()
        };
        assert_eq!(
            find(&checks(&one), "customer-emails").finding,
            "1 customer who owes you has no email address."
        );
        let two = Facts {
            customers_owing_without_email: 2,
            ..healthy()
        };
        let c = find(&checks(&two), "customer-emails");
        assert_eq!(c.status, HealthStatus::Recommendation);
        assert_eq!(c.finding, "2 customers who owe you have no email address.");
    }

    #[test]
    fn receipts_matter_most_for_gst_claims() {
        let some = Facts {
            expenses_without_receipt: 3,
            ..healthy()
        };
        let c = find(&checks(&some), "receipts");
        assert_eq!(c.status, HealthStatus::Recommendation);
        assert_eq!(c.finding, "3 expenses this financial year have no receipt.");
        assert_eq!(c.link.as_deref(), Some("/expenses?receipt=missing"));

        let gst = Facts {
            gst_claims_without_receipt: 1,
            ..some
        };
        let c = find(&checks(&gst), "receipts");
        assert_eq!(c.status, HealthStatus::Warning);
        assert_eq!(
            c.finding,
            "3 expenses this financial year have no receipt, including one that claims GST over $50."
        );
        let two = Facts {
            gst_claims_without_receipt: 2,
            ..gst
        };
        assert!(
            find(&checks(&two), "receipts")
                .finding
                .ends_with("including 2 that claim GST over $50.")
        );
        let single = Facts {
            expenses_without_receipt: 1,
            gst_claims_without_receipt: 1,
            ..healthy()
        };
        assert_eq!(
            find(&checks(&single), "receipts").finding,
            "1 expense this financial year has no receipt, including one that claims GST over $50."
        );
    }
}
