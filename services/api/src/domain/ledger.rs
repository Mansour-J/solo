//! Journal entries as pure data: what each financial event posts (docs/architecture/financial-model.md).
//! The ledger service turns these specs into rows and reconciles them with what is already posted.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::money::{percentage_of, round_to_minor};

/// Accounts every business has, addressed by their stable `system_key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemAccount {
    Bank,
    AccountsReceivable,
    Gst,
    OwnerFunds,
    OwnerDrawings,
    Sales,
    OtherIncome,
    /// Expense account for mileage claims.
    Mileage,
    /// Property, plant and equipment at book value.
    FixedAssets,
    /// Tax customers withheld from payments (schedular payments), a credit for the owner.
    WithholdingTax,
    /// Expense account for invoices written off as bad debts.
    BadDebts,
    /// Expense account for the home office claimed by IRD's square metre rate.
    HomeOffice,
    /// Income tax the owner paid to IRD from the business account.
    IncomeTax,
    /// Other owner balances brought forward (an accountant's "personal accounts").
    PersonalAccounts,
    RetainedEarnings,
    Depreciation,
    /// Income account for depreciation recovered when an asset sells above its book value.
    DepreciationRecovered,
    /// Expense account for the book value not recovered when an asset is sold or scrapped.
    LossOnDisposal,
}

impl SystemAccount {
    pub fn key(self) -> &'static str {
        match self {
            Self::Bank => "bank",
            Self::AccountsReceivable => "accounts_receivable",
            Self::Gst => "gst",
            Self::OwnerFunds => "owner_funds",
            Self::OwnerDrawings => "owner_drawings",
            Self::Sales => "sales",
            Self::OtherIncome => "other_income",
            Self::Mileage => "mileage",
            Self::FixedAssets => "fixed_assets",
            Self::WithholdingTax => "withholding_tax",
            Self::BadDebts => "bad_debts",
            Self::HomeOffice => "home_office",
            Self::IncomeTax => "income_tax",
            Self::PersonalAccounts => "personal_accounts",
            Self::RetainedEarnings => "retained_earnings",
            Self::Depreciation => "depreciation",
            Self::DepreciationRecovered => "depreciation_recovered",
            Self::LossOnDisposal => "loss_on_disposal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountRef {
    System(SystemAccount),
    Id(Uuid),
}

/// Report basis an entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntryBasis {
    Both,
    Accrual,
    Cash,
}

impl EntryBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Accrual => "accrual",
            Self::Cash => "cash",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSpec {
    pub account: AccountRef,
    pub debit: i64,
    pub credit: i64,
    pub customer_id: Option<Uuid>,
}

impl LineSpec {
    pub fn debit(account: AccountRef, amount: i64) -> Self {
        Self {
            account,
            debit: amount,
            credit: 0,
            customer_id: None,
        }
    }

    pub fn credit(account: AccountRef, amount: i64) -> Self {
        Self {
            account,
            debit: 0,
            credit: amount,
            customer_id: None,
        }
    }

    pub fn customer(mut self, customer_id: Option<Uuid>) -> Self {
        self.customer_id = customer_id;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySpec {
    pub date: NaiveDate,
    pub basis: EntryBasis,
    pub description: String,
    pub lines: Vec<LineSpec>,
}

impl EntrySpec {
    /// Builds an entry, dropping zero lines (e.g. GST on a zero-rated sale).
    fn new(date: NaiveDate, basis: EntryBasis, description: String, lines: Vec<LineSpec>) -> Self {
        Self {
            date,
            basis,
            description,
            lines: lines
                .into_iter()
                .filter(|l| l.debit != 0 || l.credit != 0)
                .collect(),
        }
    }

    /// Balanced, at least two lines, each with exactly one positive side.
    pub fn is_valid(&self) -> bool {
        let debits: i64 = self.lines.iter().map(|l| l.debit).sum();
        let credits: i64 = self.lines.iter().map(|l| l.credit).sum();
        self.lines.len() >= 2
            && debits == credits
            && self
                .lines
                .iter()
                .all(|l| (l.debit > 0) != (l.credit > 0) && l.debit >= 0 && l.credit >= 0)
    }
}

use AccountRef::System as Sys;
use SystemAccount::*;

/// Issuing an invoice (accrual basis): the customer owes the total.
pub fn invoice_issued(
    date: NaiveDate,
    number: &str,
    customer: Uuid,
    net: i64,
    gst: i64,
) -> EntrySpec {
    EntrySpec::new(
        date,
        EntryBasis::Accrual,
        format!("Invoice {number}"),
        vec![
            LineSpec::debit(Sys(AccountsReceivable), net + gst).customer(Some(customer)),
            LineSpec::credit(Sys(Sales), net).customer(Some(customer)),
            LineSpec::credit(Sys(Gst), gst),
        ],
    )
}

/// The share of an invoice's net and GST that cumulative payments represent, so that the cash-basis
/// recognition of all payments adds up exactly to the invoice once it is fully paid.
fn recognised(net: i64, gst: i64, total: i64, paid: i64) -> (i64, i64) {
    if total == 0 || paid >= total {
        return (net, gst);
    }
    let share = |amount: i64| {
        round_to_minor(Decimal::from(amount) * Decimal::from(paid) / Decimal::from(total))
            .unwrap_or(0)
    };
    let gst_part = share(gst);
    (paid - gst_part, gst_part)
}

/// A payment against an invoice: settles the receivable (accrual) and recognises income and GST in
/// proportion to the invoice (cash). `paid_before` is the total of earlier active payments.
/// `withholding_tax` is the part of `amount` the customer paid to IRD for the owner instead of to the
/// bank (schedular payments): it settles the invoice like the rest and is a tax credit for the owner.
#[allow(clippy::too_many_arguments)]
pub fn invoice_payment(
    date: NaiveDate,
    number: &str,
    customer: Uuid,
    amount: i64,
    withholding_tax: i64,
    invoice_net: i64,
    invoice_gst: i64,
    paid_before: i64,
) -> [EntrySpec; 2] {
    let total = invoice_net + invoice_gst;
    let (net_before, gst_before) = recognised(invoice_net, invoice_gst, total, paid_before);
    let (net_after, gst_after) = recognised(invoice_net, invoice_gst, total, paid_before + amount);
    let description = format!("Payment for invoice {number}");
    let received = amount - withholding_tax;
    [
        EntrySpec::new(
            date,
            EntryBasis::Accrual,
            description.clone(),
            vec![
                LineSpec::debit(Sys(Bank), received),
                LineSpec::debit(Sys(WithholdingTax), withholding_tax),
                LineSpec::credit(Sys(AccountsReceivable), amount).customer(Some(customer)),
            ],
        ),
        EntrySpec::new(
            date,
            EntryBasis::Cash,
            description,
            vec![
                LineSpec::debit(Sys(Bank), received),
                LineSpec::debit(Sys(WithholdingTax), withholding_tax),
                LineSpec::credit(Sys(Sales), net_after - net_before).customer(Some(customer)),
                LineSpec::credit(Sys(Gst), gst_after - gst_before),
            ],
        ),
    ]
}

/// Writing off what's left of an invoice as a bad debt (accrual only: on the cash basis the sale was
/// never counted, so there's nothing to reverse). The receivable is settled by a bad-debt expense and,
/// for the GST in it, a credit adjustment of the GST returned on the sale (IR375: 3/23 of a
/// GST-inclusive amount written off), dated when it's written off.
pub fn invoice_write_off(
    date: NaiveDate,
    number: &str,
    customer: Uuid,
    amount: i64,
    invoice_net: i64,
    invoice_gst: i64,
    paid_before: i64,
) -> EntrySpec {
    let total = invoice_net + invoice_gst;
    let (_, gst_before) = recognised(invoice_net, invoice_gst, total, paid_before);
    let (_, gst_after) = recognised(invoice_net, invoice_gst, total, paid_before + amount);
    let gst = gst_after - gst_before;
    EntrySpec::new(
        date,
        EntryBasis::Accrual,
        format!("Bad debt written off: invoice {number}"),
        vec![
            LineSpec::debit(Sys(BadDebts), amount - gst),
            LineSpec::debit(Sys(Gst), gst),
            LineSpec::credit(Sys(AccountsReceivable), amount).customer(Some(customer)),
        ],
    )
}

/// A year's home office claim (both bases): costs of the owner's home the business used, paid by the
/// owner personally, so the claim comes in as funds introduced.
pub fn home_office_claim(date: NaiveDate, claim: i64) -> EntrySpec {
    EntrySpec::new(
        date,
        EntryBasis::Both,
        "Home office (square metre rate)".into(),
        vec![
            LineSpec::debit(Sys(HomeOffice), claim),
            LineSpec::credit(Sys(OwnerFunds), claim),
        ],
    )
}

/// A GST change-of-use adjustment for an asset, at the end of an adjustment period (both bases):
/// GST claimed for more business use than intended is money the owner put in; GST paid back for
/// private use is drawings, as the private share of depreciation is.
pub fn asset_use_adjustment(date: NaiveDate, name: &str, adjustment: i64) -> EntrySpec {
    let name: String = name.chars().take(250).collect();
    let lines = if adjustment >= 0 {
        vec![
            LineSpec::debit(Sys(Gst), adjustment),
            LineSpec::credit(Sys(OwnerFunds), adjustment),
        ]
    } else {
        vec![
            LineSpec::debit(Sys(OwnerDrawings), -adjustment),
            LineSpec::credit(Sys(Gst), -adjustment),
        ]
    };
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("GST change-of-use adjustment: {name}"),
        lines,
    )
}

/// An asset sold or scrapped (both bases): the money received (with any GST on the sale) against the
/// book value, with depreciation recovered or the loss, the private-use share and any capital gain
/// with the owner, and IRD's final change-of-use credit (`final_gst_credit`) where one applies.
pub fn asset_disposal(
    date: NaiveDate,
    name: &str,
    sale_price: i64,
    sale_gst: i64,
    book_value: i64,
    outcome: &super::depreciation::Disposal,
    final_gst_credit: i64,
) -> EntrySpec {
    let name: String = name.chars().take(250).collect();
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("Asset sold or scrapped: {name}"),
        vec![
            LineSpec::debit(Sys(Bank), sale_price),
            LineSpec::credit(Sys(Gst), sale_gst),
            LineSpec::credit(Sys(FixedAssets), book_value),
            LineSpec::credit(Sys(DepreciationRecovered), outcome.recovered),
            LineSpec::credit(
                Sys(OwnerFunds),
                outcome.recovered_private + outcome.capital_gain,
            ),
            LineSpec::debit(Sys(LossOnDisposal), outcome.loss),
            LineSpec::debit(Sys(OwnerDrawings), outcome.loss_private),
            LineSpec::debit(Sys(Gst), final_gst_credit),
            LineSpec::credit(Sys(OwnerFunds), final_gst_credit),
        ],
    )
}

/// Money the owner or IRD moved in or out of the business account (both bases).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MovementKind {
    /// The owner took money out.
    Drawing,
    /// The owner put money in.
    FundsIntroduced,
    /// GST paid to IRD.
    GstPayment,
    /// A GST refund from IRD.
    GstRefund,
    /// Income tax paid to IRD from the business account.
    IncomeTaxPayment,
    IncomeTaxRefund,
}

impl MovementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Drawing => "drawing",
            Self::FundsIntroduced => "funds_introduced",
            Self::GstPayment => "gst_payment",
            Self::GstRefund => "gst_refund",
            Self::IncomeTaxPayment => "income_tax_payment",
            Self::IncomeTaxRefund => "income_tax_refund",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "drawing" => Self::Drawing,
            "funds_introduced" => Self::FundsIntroduced,
            "gst_payment" => Self::GstPayment,
            "gst_refund" => Self::GstRefund,
            "income_tax_payment" => Self::IncomeTaxPayment,
            "income_tax_refund" => Self::IncomeTaxRefund,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Drawing => "Drawings",
            Self::FundsIntroduced => "Funds introduced",
            Self::GstPayment => "GST paid to IRD",
            Self::GstRefund => "GST refund",
            Self::IncomeTaxPayment => "Income tax paid",
            Self::IncomeTaxRefund => "Income tax refund",
        }
    }

    /// Whether the money comes into the business account.
    pub fn is_inflow(self) -> bool {
        matches!(
            self,
            Self::FundsIntroduced | Self::GstRefund | Self::IncomeTaxRefund
        )
    }
}

pub fn cash_movement(
    date: NaiveDate,
    kind: MovementKind,
    amount: i64,
    description: &str,
) -> EntrySpec {
    let other = match kind {
        MovementKind::Drawing => Sys(OwnerDrawings),
        MovementKind::FundsIntroduced => Sys(OwnerFunds),
        MovementKind::GstPayment | MovementKind::GstRefund => Sys(Gst),
        MovementKind::IncomeTaxPayment | MovementKind::IncomeTaxRefund => Sys(IncomeTax),
    };
    let detail: String = description.chars().take(250).collect();
    let text = if detail.is_empty() {
        kind.label().to_owned()
    } else {
        format!("{}: {detail}", kind.label())
    };
    let lines = if kind.is_inflow() {
        vec![
            LineSpec::debit(Sys(Bank), amount),
            LineSpec::credit(other, amount),
        ]
    } else {
        vec![
            LineSpec::debit(other, amount),
            LineSpec::credit(Sys(Bank), amount),
        ]
    };
    EntrySpec::new(date, EntryBasis::Both, text, lines)
}

/// Buying a fixed asset after the opening balances (both bases): its cost goes to property, plant
/// and equipment, not to expenses; GST paid on it is claimed.
pub fn asset_purchase(date: NaiveDate, name: &str, cost: i64, gst: i64) -> EntrySpec {
    let name: String = name.chars().take(250).collect();
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("Asset bought: {name}"),
        vec![
            LineSpec::debit(Sys(FixedAssets), cost),
            LineSpec::debit(Sys(Gst), gst),
            LineSpec::credit(Sys(Bank), cost + gst),
        ],
    )
}

/// A year's depreciation of an asset (both bases), dated the last day of the financial year: the
/// business share is an expense, the private-use share goes to drawings.
pub fn depreciation(
    date: NaiveDate,
    name: &str,
    depreciation: i64,
    private_amount: i64,
) -> EntrySpec {
    let name: String = name.chars().take(250).collect();
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("Depreciation: {name}"),
        vec![
            LineSpec::debit(Sys(Depreciation), depreciation - private_amount),
            LineSpec::debit(Sys(OwnerDrawings), private_amount),
            LineSpec::credit(Sys(FixedAssets), depreciation),
        ],
    )
}

/// Balances brought forward, in minor units. Assets are positive; `gst_payable` is owed to IRD
/// (negative for a refund due); equity is as on a balance sheet (drawings and taxes paid negative).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Opening {
    pub bank: i64,
    pub fixed_assets: i64,
    pub gst_payable: i64,
    pub drawings: i64,
    pub funds_introduced: i64,
    pub personal_accounts: i64,
    pub withholding_tax: i64,
    pub income_tax: i64,
}

impl Opening {
    /// Net assets less the equity given: what retained earnings must be for the balances to balance.
    pub fn retained_earnings(&self) -> i64 {
        self.bank + self.fixed_assets
            - self.gst_payable
            - (self.drawings
                + self.funds_introduced
                + self.personal_accounts
                + self.withholding_tax
                + self.income_tax)
    }
}

/// An amount on its natural side: assets on the debit side when positive, equity and liabilities on
/// the credit side.
fn signed(account: AccountRef, amount: i64, debit_natural: bool) -> LineSpec {
    match (amount >= 0, debit_natural) {
        (true, true) | (false, false) => LineSpec::debit(account, amount.abs()),
        _ => LineSpec::credit(account, amount.abs()),
    }
}

/// The opening balances as one entry (both bases); retained earnings balances it.
pub fn opening_balances(date: NaiveDate, o: &Opening) -> EntrySpec {
    EntrySpec::new(
        date,
        EntryBasis::Both,
        "Opening balances".into(),
        vec![
            signed(Sys(Bank), o.bank, true),
            signed(Sys(FixedAssets), o.fixed_assets, true),
            signed(Sys(Gst), o.gst_payable, false),
            signed(Sys(OwnerDrawings), o.drawings, false),
            signed(Sys(OwnerFunds), o.funds_introduced, false),
            signed(Sys(PersonalAccounts), o.personal_accounts, false),
            signed(Sys(WithholdingTax), o.withholding_tax, false),
            signed(Sys(IncomeTax), o.income_tax, false),
            signed(Sys(RetainedEarnings), o.retained_earnings(), false),
        ],
    )
}

/// How an expense divides between the business, claimable GST and the owner's private share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct ExpenseSplit {
    /// Business share posted to the expense category.
    pub expense_amount: i64,
    /// GST claimable from IRD (business share of GST, when registered).
    pub gst_claim: i64,
    /// Private share, recorded as owner drawings.
    pub private_amount: i64,
}

pub fn split_expense(
    gross: i64,
    gst: i64,
    business_percentage: Decimal,
    gst_claimable: bool,
) -> ExpenseSplit {
    let (expense_amount, gst_claim) = if gst_claimable {
        (
            percentage_of(gross - gst, business_percentage).unwrap_or(0),
            percentage_of(gst, business_percentage).unwrap_or(0),
        )
    } else {
        (percentage_of(gross, business_percentage).unwrap_or(0), 0)
    };
    ExpenseSplit {
        expense_amount,
        gst_claim,
        private_amount: gross - expense_amount - gst_claim,
    }
}

/// An expense (both bases: recorded expenses are already paid).
pub fn expense(
    date: NaiveDate,
    supplier: &str,
    expense_account: Uuid,
    gross: i64,
    split: ExpenseSplit,
    paid_personally: bool,
) -> EntrySpec {
    let business = AccountRef::Id(expense_account);
    let lines = if paid_personally {
        // Paid from the owner's own money: only the business share comes into the business, as funds
        // introduced. The private share never touched the business, so it isn't a drawing either.
        vec![
            LineSpec::debit(business, split.expense_amount),
            LineSpec::debit(Sys(Gst), split.gst_claim),
            LineSpec::credit(Sys(OwnerFunds), split.expense_amount + split.gst_claim),
        ]
    } else {
        vec![
            LineSpec::debit(business, split.expense_amount),
            LineSpec::debit(Sys(Gst), split.gst_claim),
            LineSpec::debit(Sys(OwnerDrawings), split.private_amount),
            LineSpec::credit(Sys(Bank), gross),
        ]
    };
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("Expense: {supplier}"),
        lines,
    )
}

/// Income received without an invoice (both bases).
pub fn income(
    date: NaiveDate,
    payer: &str,
    other_income: bool,
    customer: Option<Uuid>,
    net: i64,
    gst: i64,
) -> EntrySpec {
    let account = if other_income {
        Sys(OtherIncome)
    } else {
        Sys(Sales)
    };
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("Income: {payer}"),
        vec![
            LineSpec::debit(Sys(Bank), net + gst),
            LineSpec::credit(account, net).customer(customer),
            LineSpec::credit(Sys(Gst), gst),
        ],
    )
}

/// A mileage claim (both bases): the business's cost of using a vehicle the owner paid for.
pub fn mileage_claim(date: NaiveDate, purpose: &str, claim: i64) -> EntrySpec {
    // Entry descriptions are limited to 300 characters; a purpose can be up to 300 on its own.
    let purpose: String = purpose.chars().take(280).collect();
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("Mileage: {purpose}"),
        vec![
            LineSpec::debit(Sys(Mileage), claim),
            LineSpec::credit(Sys(OwnerFunds), claim),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn system_accounts_have_distinct_keys_and_bases_have_names() {
        use SystemAccount::*;
        let all = [
            Bank,
            AccountsReceivable,
            Gst,
            OwnerFunds,
            OwnerDrawings,
            Sales,
            OtherIncome,
            Mileage,
            BadDebts,
            HomeOffice,
            FixedAssets,
            WithholdingTax,
            IncomeTax,
            PersonalAccounts,
            RetainedEarnings,
            Depreciation,
            DepreciationRecovered,
            LossOnDisposal,
        ];
        let keys: std::collections::BTreeSet<&str> = all.iter().map(|a| a.key()).collect();
        assert_eq!(keys.len(), all.len());
        assert!(keys.contains("retained_earnings") && keys.contains("withholding_tax"));
        assert_eq!(
            [EntryBasis::Both, EntryBasis::Accrual, EntryBasis::Cash].map(EntryBasis::as_str),
            ["both", "accrual", "cash"]
        );
    }

    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")
    }

    #[test]
    fn mileage_claims_balance_and_fit_the_description_limit() {
        let entry = mileage_claim(d(), &"x".repeat(300), 4_420);
        assert!(entry.is_valid());
        assert_eq!(entry.lines[0].account, Sys(Mileage));
        assert_eq!(entry.lines[1].account, Sys(OwnerFunds));
        assert!(entry.description.chars().count() <= 300);
    }

    #[test]
    fn invoice_entry_balances_and_skips_zero_gst() {
        let customer = Uuid::now_v7();
        let entry = invoice_issued(d(), "INV-0001", customer, 10_000, 1_500);
        assert!(entry.is_valid());
        assert_eq!(entry.lines.len(), 3);
        let zero_rated = invoice_issued(d(), "INV-0002", customer, 10_000, 0);
        assert!(zero_rated.is_valid());
        assert_eq!(zero_rated.lines.len(), 2);
    }

    #[test]
    fn partial_payments_recognise_exactly_the_invoice_amounts() {
        let customer = Uuid::now_v7();
        // $333.33 + GST $50.00 = $383.33, paid in three uneven parts.
        let (net, gst) = (33_333, 5_000);
        let mut paid = 0;
        let (mut sales, mut tax) = (0, 0);
        for amount in [10_000, 12_345, 15_988] {
            let [accrual, cash] =
                invoice_payment(d(), "INV-1", customer, amount, 0, net, gst, paid);
            assert!(accrual.is_valid() && cash.is_valid());
            for line in &cash.lines {
                match line.account {
                    Sys(Sales) => sales += line.credit,
                    Sys(Gst) => tax += line.credit,
                    _ => {}
                }
            }
            paid += amount;
        }
        assert_eq!(paid, net + gst);
        assert_eq!((sales, tax), (net, gst));
    }

    fn amount_on(entry: &EntrySpec, account: SystemAccount) -> (i64, i64) {
        entry
            .lines
            .iter()
            .filter(|l| l.account == Sys(account))
            .fold((0, 0), |(d, c), l| (d + l.debit, c + l.credit))
    }

    #[test]
    fn withholding_tax_settles_the_invoice_but_never_reaches_the_bank() {
        // $1,150 incl. GST; the customer withheld 20% of the $1,000 excl. GST.
        let customer = Uuid::now_v7();
        let [accrual, cash] =
            invoice_payment(d(), "INV-7", customer, 115_000, 20_000, 100_000, 15_000, 0);
        assert!(accrual.is_valid() && cash.is_valid());
        for entry in [&accrual, &cash] {
            assert_eq!(amount_on(entry, Bank), (95_000, 0));
            assert_eq!(amount_on(entry, WithholdingTax), (20_000, 0));
        }
        assert_eq!(amount_on(&accrual, AccountsReceivable), (0, 115_000));
        assert_eq!(amount_on(&cash, Sales), (0, 100_000));
        assert_eq!(amount_on(&cash, Gst), (0, 15_000));
    }

    #[test]
    fn money_moved_in_and_out_uses_the_matching_account() {
        let cases = [
            (MovementKind::Drawing, OwnerDrawings, false),
            (MovementKind::FundsIntroduced, OwnerFunds, true),
            (MovementKind::GstPayment, Gst, false),
            (MovementKind::GstRefund, Gst, true),
            (MovementKind::IncomeTaxPayment, IncomeTax, false),
            (MovementKind::IncomeTaxRefund, IncomeTax, true),
        ];
        for (kind, account, inflow) in cases {
            assert_eq!(MovementKind::parse(kind.as_str()), Some(kind));
            assert_eq!(kind.is_inflow(), inflow);
            let entry = cash_movement(d(), kind, 5_000, "");
            assert!(entry.is_valid());
            assert_eq!(entry.basis, EntryBasis::Both);
            assert_eq!(entry.description, kind.label());
            let expected = if inflow {
                ((5_000, 0), (0, 5_000))
            } else {
                ((0, 5_000), (5_000, 0))
            };
            assert_eq!(
                (amount_on(&entry, Bank), amount_on(&entry, account)),
                expected,
                "{kind:?}"
            );
        }
        assert_eq!(MovementKind::parse("bonus"), None);
        assert_eq!(
            cash_movement(d(), MovementKind::GstPayment, 100, "Return to 31 May").description,
            "GST paid to IRD: Return to 31 May"
        );
    }

    #[test]
    fn assets_and_depreciation_move_between_the_balance_sheet_and_expenses() {
        let bought = asset_purchase(d(), "Laptop", 200_000, 30_000);
        assert!(bought.is_valid());
        assert_eq!(amount_on(&bought, FixedAssets), (200_000, 0));
        assert_eq!(amount_on(&bought, Gst), (30_000, 0));
        assert_eq!(amount_on(&bought, Bank), (0, 230_000));
        let year = depreciation(d(), "Laptop", 40_000, 12_000);
        assert!(year.is_valid());
        assert_eq!(amount_on(&year, Depreciation), (28_000, 0));
        assert_eq!(amount_on(&year, OwnerDrawings), (12_000, 0));
        assert_eq!(amount_on(&year, FixedAssets), (0, 40_000));
        // No private use: two lines.
        assert_eq!(depreciation(d(), "Desk", 5_000, 0).lines.len(), 2);
    }

    #[test]
    fn opening_balances_balance_on_retained_earnings() {
        // A comparative balance sheet at 31 March 2024 (made-up figures).
        let opening = Opening {
            bank: 4_825_000,
            fixed_assets: 433_865,
            gst_payable: 213_450,
            drawings: -9_640_000,
            funds_introduced: 1_250_000,
            personal_accounts: -320_000,
            withholding_tax: -5_470_000,
            income_tax: 0,
        };
        // Net assets of $50,454.15 ($48,250 + $4,338.65 − $2,134.50), less the owner’s balances.
        assert_eq!(opening.retained_earnings(), 19_225_415);
        let entry = opening_balances(d(), &opening);
        assert!(entry.is_valid());
        assert_eq!(amount_on(&entry, Bank), (4_825_000, 0));
        assert_eq!(amount_on(&entry, Gst), (0, 213_450));
        assert_eq!(amount_on(&entry, OwnerDrawings), (9_640_000, 0));
        assert_eq!(amount_on(&entry, OwnerFunds), (0, 1_250_000));
        assert_eq!(amount_on(&entry, RetainedEarnings), (0, 19_225_415));
        assert_eq!(entry.lines.len(), 8, "the zero income tax line is dropped");
        // An overdraft and a GST refund due sit on the other sides.
        let other = opening_balances(
            d(),
            &Opening {
                bank: -1_000,
                gst_payable: -500,
                ..Opening::default()
            },
        );
        assert!(other.is_valid());
        assert_eq!(amount_on(&other, Bank), (0, 1_000));
        assert_eq!(amount_on(&other, Gst), (500, 0));
        assert_eq!(amount_on(&other, RetainedEarnings), (500, 0));
    }

    #[test]
    fn expense_splits() {
        // $115 inclusive of $15 GST, 60% business, GST-registered.
        let split = split_expense(11_500, 1_500, dec!(60), true);
        assert_eq!(
            split,
            ExpenseSplit {
                expense_amount: 6_000,
                gst_claim: 900,
                private_amount: 4_600
            }
        );
        // Not registered: GST is part of the cost.
        let split = split_expense(11_500, 1_500, dec!(100), false);
        assert_eq!(
            split,
            ExpenseSplit {
                expense_amount: 11_500,
                gst_claim: 0,
                private_amount: 0
            }
        );
        // Personal expense: everything is drawings.
        let split = split_expense(5_000, 652, dec!(0), true);
        assert_eq!(
            split,
            ExpenseSplit {
                expense_amount: 0,
                gst_claim: 0,
                private_amount: 5_000
            }
        );
        let entry = expense(
            d(),
            "Spark",
            Uuid::now_v7(),
            11_500,
            split_expense(11_500, 1_500, dec!(60), true),
            true,
        );
        assert!(entry.is_valid());
        // Paid personally: only the business share ($60 + $9 GST) is funds introduced; the private
        // $46 never went through the business, so it isn't a drawing.
        assert_eq!(amount_on(&entry, OwnerFunds), (0, 6_900));
        assert_eq!(amount_on(&entry, OwnerDrawings), (0, 0));
        // Paid from the business account: the private share is a drawing.
        let from_bank = expense(
            d(),
            "Spark",
            Uuid::now_v7(),
            11_500,
            split_expense(11_500, 1_500, dec!(60), true),
            false,
        );
        assert_eq!(amount_on(&from_bank, Bank), (0, 11_500));
        assert_eq!(amount_on(&from_bank, OwnerDrawings), (4_600, 0));
        // Wholly private and paid personally: nothing for the business to record.
        let private = expense(
            d(),
            "Countdown",
            Uuid::now_v7(),
            5_000,
            split_expense(5_000, 652, dec!(0), true),
            true,
        );
        assert!(private.lines.is_empty());
    }

    #[test]
    fn income_entry_balances() {
        assert!(income(d(), "Market stall", false, None, 8_696, 1_304).is_valid());
        assert!(income(d(), "Interest", true, None, 1_234, 0).is_valid());
    }
}
