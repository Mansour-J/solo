//! Allocation plans: how a receipt would be split across destinations by the business's rules
//! (ADR 0010). Pure: the service supplies the rules, balances and amounts already allocated.
//!
//! Rules apply in priority order, each to what is left: a percentage (of the amount received or of
//! what remains) or a fixed amount, then limited by the rule's maximum, its cap for the period, the
//! destination's target (when the rule stops there) and what remains. Every amount is whole minor
//! units, so the planned amounts plus the remainder always equal the amount received.

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::money::percentage_of;

/// What kind of receipt is being allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptSource {
    InvoicePayment,
    /// Income recorded without an invoice, with its type (`sales` or `other`).
    Income {
        sales: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receipt {
    pub amount: i64,
    pub date: NaiveDate,
    pub source: ReceiptSource,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// The percentage is of the amount received.
    Received,
    /// The percentage is of what earlier rules left.
    Remaining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAmount {
    Percentage { percentage: Decimal, basis: Basis },
    Fixed(i64),
}

/// Which receipts a rule applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCondition {
    Any,
    InvoicePayments,
    /// Income records, optionally only sales (`Some(true)`) or only other income (`Some(false)`).
    Income {
        sales: Option<bool>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapPeriod {
    /// Monday to Sunday.
    Week,
    /// Fourteen days, counted from the anchor date.
    Fortnight { anchor: NaiveDate },
    /// A calendar month.
    Month,
    /// `days` days, counted from the anchor date.
    Custom { anchor: NaiveDate, days: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cap {
    pub amount: i64,
    pub period: CapPeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub id: Uuid,
    pub destination_id: Uuid,
    pub amount: RuleAmount,
    pub source: SourceCondition,
    pub customer_id: Option<Uuid>,
    pub active_from: Option<NaiveDate>,
    pub active_to: Option<NaiveDate>,
    /// Most this rule allocates from one receipt.
    pub max_amount: Option<i64>,
    pub cap: Option<Cap>,
    /// Stop once the destination's balance reaches its target.
    pub stop_at_target: bool,
}

impl Rule {
    /// Whether the rule applies to the receipt (source, customer and active dates).
    pub fn applies_to(&self, receipt: &Receipt) -> bool {
        let source_ok = match (self.source, receipt.source) {
            (SourceCondition::Any, _) => true,
            (SourceCondition::InvoicePayments, ReceiptSource::InvoicePayment) => true,
            (SourceCondition::Income { sales: None }, ReceiptSource::Income { .. }) => true,
            (SourceCondition::Income { sales: Some(want) }, ReceiptSource::Income { sales }) => {
                want == sales
            }
            _ => false,
        };
        source_ok
            && self
                .customer_id
                .is_none_or(|c| receipt.customer_id == Some(c))
            && self.active_from.is_none_or(|from| receipt.date >= from)
            && self.active_to.is_none_or(|to| receipt.date <= to)
    }
}

/// The first and last day (inclusive) of the cap period containing `date`.
pub fn period_containing(period: CapPeriod, date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let counted = |anchor: NaiveDate, days: i64| {
        let offset = (date - anchor).num_days().div_euclid(days) * days;
        let start = anchor + Duration::days(offset);
        (start, start + Duration::days(days - 1))
    };
    match period {
        CapPeriod::Week => {
            let start = date - Duration::days(i64::from(date.weekday().num_days_from_monday()));
            (start, start + Duration::days(6))
        }
        CapPeriod::Fortnight { anchor } => counted(anchor, 14),
        CapPeriod::Custom { anchor, days } => counted(anchor, i64::from(days.max(1))),
        CapPeriod::Month => {
            let start = date.with_day(1).unwrap_or(date);
            let next = if start.month() == 12 {
                NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
            };
            (start, next.map_or(date, |n| n - Duration::days(1)))
        }
    }
}

/// What the plan knows about the books besides the rules.
#[derive(Debug, Clone, Default)]
pub struct PlanContext {
    /// Current balance of each destination (confirmed entries, excluding this receipt).
    pub balances: HashMap<Uuid, i64>,
    /// Target of each destination that has one.
    pub targets: HashMap<Uuid, i64>,
    /// Amount each capped rule already allocated in the cap period containing the receipt date
    /// (excluding this receipt).
    pub cap_used: HashMap<Uuid, i64>,
}

/// Why a rule planned less than it asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Limit {
    /// The rule's maximum per receipt.
    Maximum,
    /// The rule's cap for the period.
    Cap,
    /// The destination reached its target.
    Target,
    /// Earlier rules used the rest of the receipt.
    Remaining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedLine {
    pub rule_id: Uuid,
    pub destination_id: Uuid,
    /// What the rule asked for before limits.
    pub requested: i64,
    pub amount: i64,
    /// The first limit that reduced the amount, if any.
    pub limited_by: Option<Limit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub lines: Vec<PlannedLine>,
    pub allocated: i64,
    pub remaining: i64,
}

/// Plans a receipt. `rules` must already be in priority order; rules that don't apply are left out.
pub fn plan(receipt: &Receipt, rules: &[Rule], context: &PlanContext) -> Plan {
    let mut remaining = receipt.amount.max(0);
    let mut added: HashMap<Uuid, i64> = HashMap::new();
    let mut lines = Vec::new();
    for rule in rules.iter().filter(|r| r.applies_to(receipt)) {
        let requested = match rule.amount {
            RuleAmount::Percentage { percentage, basis } => {
                let base = match basis {
                    Basis::Received => receipt.amount,
                    Basis::Remaining => remaining,
                };
                percentage_of(base, percentage).unwrap_or(0).max(0)
            }
            RuleAmount::Fixed(amount) => amount.max(0),
        };
        let mut amount = requested;
        let mut limited_by = None;
        let mut limit = |amount: &mut i64, ceiling: i64, reason: Limit| {
            let ceiling = ceiling.max(0);
            if *amount > ceiling {
                *amount = ceiling;
                limited_by.get_or_insert(reason);
            }
        };
        if let Some(max) = rule.max_amount {
            limit(&mut amount, max, Limit::Maximum);
        }
        if let Some(cap) = rule.cap {
            let used = context.cap_used.get(&rule.id).copied().unwrap_or(0);
            limit(&mut amount, cap.amount - used, Limit::Cap);
        }
        if rule.stop_at_target
            && let Some(target) = context.targets.get(&rule.destination_id)
        {
            let balance = context
                .balances
                .get(&rule.destination_id)
                .copied()
                .unwrap_or(0)
                + added.get(&rule.destination_id).copied().unwrap_or(0);
            limit(&mut amount, target - balance, Limit::Target);
        }
        limit(&mut amount, remaining, Limit::Remaining);
        remaining -= amount;
        *added.entry(rule.destination_id).or_default() += amount;
        lines.push(PlannedLine {
            rule_id: rule.id,
            destination_id: rule.destination_id,
            requested,
            amount,
            limited_by,
        });
    }
    let allocated = receipt.amount.max(0) - remaining;
    Plan {
        lines,
        allocated,
        remaining,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn receipt(amount: i64) -> Receipt {
        Receipt {
            amount,
            date: d(2026, 6, 10),
            source: ReceiptSource::InvoicePayment,
            customer_id: None,
        }
    }

    fn percent(n: u128, destination: u128, percentage: Decimal, basis: Basis) -> Rule {
        Rule {
            id: id(n),
            destination_id: id(destination),
            amount: RuleAmount::Percentage { percentage, basis },
            source: SourceCondition::Any,
            customer_id: None,
            active_from: None,
            active_to: None,
            max_amount: None,
            cap: None,
            stop_at_target: false,
        }
    }

    fn fixed(n: u128, destination: u128, amount: i64) -> Rule {
        Rule {
            amount: RuleAmount::Fixed(amount),
            ..percent(n, destination, dec!(1), Basis::Received)
        }
    }

    fn amounts(plan: &Plan) -> Vec<i64> {
        plan.lines.iter().map(|l| l.amount).collect()
    }

    #[test]
    fn percentages_of_the_amount_received_and_of_what_remains() {
        // $1,000.00: 20% of the receipt for tax ($200), $100 fixed, then 10% of the $700 left ($70).
        let rules = [
            percent(1, 10, dec!(20), Basis::Received),
            fixed(2, 11, 10_000),
            percent(3, 12, dec!(10), Basis::Remaining),
        ];
        let plan = plan(&receipt(100_000), &rules, &PlanContext::default());
        assert_eq!(amounts(&plan), [20_000, 10_000, 7_000]);
        assert_eq!(plan.allocated, 37_000);
        assert_eq!(plan.remaining, 63_000);
    }

    #[test]
    fn parts_and_remainder_always_add_up_to_the_receipt() {
        // $333.33 at 33.33% three times: rounding never makes the parts exceed the receipt.
        let rules = [
            percent(1, 10, dec!(33.33), Basis::Received),
            percent(2, 11, dec!(33.33), Basis::Received),
            percent(3, 12, dec!(33.34), Basis::Received),
        ];
        let plan = plan(&receipt(33_333), &rules, &PlanContext::default());
        assert_eq!(amounts(&plan), [11_110, 11_110, 11_113]);
        assert_eq!(plan.allocated + plan.remaining, 33_333);
        assert_eq!(plan.remaining, 0);
    }

    #[test]
    fn a_fixed_amount_larger_than_what_remains_takes_only_the_rest() {
        let rules = [
            percent(1, 10, dec!(50), Basis::Received),
            fixed(2, 11, 80_000),
            fixed(3, 12, 1_000),
        ];
        let plan = plan(&receipt(100_000), &rules, &PlanContext::default());
        assert_eq!(amounts(&plan), [50_000, 50_000, 0]);
        assert_eq!(plan.lines[1].requested, 80_000);
        assert_eq!(plan.lines[1].limited_by, Some(Limit::Remaining));
        assert_eq!(plan.lines[2].limited_by, Some(Limit::Remaining));
        assert_eq!(plan.remaining, 0);
    }

    #[test]
    fn maximums_caps_and_targets_limit_a_rule() {
        let mut maximum = percent(1, 10, dec!(50), Basis::Received);
        maximum.max_amount = Some(20_000);
        let mut capped = percent(2, 11, dec!(10), Basis::Received);
        capped.cap = Some(Cap {
            amount: 15_000,
            period: CapPeriod::Month,
        });
        let mut targeted = percent(3, 12, dec!(10), Basis::Received);
        targeted.stop_at_target = true;
        let context = PlanContext {
            balances: HashMap::from([(id(12), 95_000)]),
            targets: HashMap::from([(id(12), 100_000)]),
            cap_used: HashMap::from([(id(2), 12_000)]),
        };
        let plan = plan(&receipt(100_000), &[maximum, capped, targeted], &context);
        // 50% → $500 limited to $200; 10% → $100 limited to the $30 left of the $150 cap; 10% →
        // $100 limited to the $50 still needed to reach the $1,000 target.
        assert_eq!(amounts(&plan), [20_000, 3_000, 5_000]);
        let limits: Vec<_> = plan.lines.iter().map(|l| l.limited_by).collect();
        assert_eq!(
            limits,
            [Some(Limit::Maximum), Some(Limit::Cap), Some(Limit::Target)]
        );
        assert_eq!(plan.remaining, 72_000);
    }

    #[test]
    fn a_target_counts_what_earlier_rules_in_the_same_plan_added() {
        let mut first = fixed(1, 10, 30_000);
        first.stop_at_target = true;
        let mut second = fixed(2, 10, 30_000);
        second.stop_at_target = true;
        let context = PlanContext {
            targets: HashMap::from([(id(10), 50_000)]),
            ..PlanContext::default()
        };
        let plan = plan(&receipt(100_000), &[first, second], &context);
        assert_eq!(amounts(&plan), [30_000, 20_000]);
        // A destination already over its target gets nothing more.
        let over = PlanContext {
            balances: HashMap::from([(id(10), 60_000)]),
            targets: HashMap::from([(id(10), 50_000)]),
            ..PlanContext::default()
        };
        assert_eq!(
            amounts(&super::plan(&receipt(100_000), &[first], &over)),
            [0]
        );
    }

    #[test]
    fn conditions_choose_the_receipts_a_rule_applies_to() {
        let customer = id(99);
        let mut payments_only = fixed(1, 10, 100);
        payments_only.source = SourceCondition::InvoicePayments;
        let mut sales_only = fixed(2, 11, 100);
        sales_only.source = SourceCondition::Income { sales: Some(true) };
        let mut for_customer = fixed(3, 12, 100);
        for_customer.customer_id = Some(customer);
        let mut june_only = fixed(4, 13, 100);
        june_only.active_from = Some(d(2026, 6, 1));
        june_only.active_to = Some(d(2026, 6, 30));
        let rules = [payments_only, sales_only, for_customer, june_only];

        let payment = Receipt {
            customer_id: Some(customer),
            ..receipt(1_000)
        };
        let applied: Vec<Uuid> = plan(&payment, &rules, &PlanContext::default())
            .lines
            .iter()
            .map(|l| l.rule_id)
            .collect();
        assert_eq!(applied, [id(1), id(3), id(4)]);

        let other_income = Receipt {
            source: ReceiptSource::Income { sales: false },
            date: d(2026, 7, 1),
            ..receipt(1_000)
        };
        assert!(
            plan(&other_income, &rules, &PlanContext::default())
                .lines
                .is_empty()
        );
        let sale = Receipt {
            source: ReceiptSource::Income { sales: true },
            ..other_income
        };
        let applied: Vec<Uuid> = plan(&sale, &rules, &PlanContext::default())
            .lines
            .iter()
            .map(|l| l.rule_id)
            .collect();
        assert_eq!(applied, [id(2)]);
    }

    #[test]
    fn cap_periods_contain_the_receipt_date() {
        // Wednesday 10 June 2026.
        let date = d(2026, 6, 10);
        assert_eq!(
            period_containing(CapPeriod::Week, date),
            (d(2026, 6, 8), d(2026, 6, 14))
        );
        assert_eq!(
            period_containing(CapPeriod::Month, date),
            (d(2026, 6, 1), d(2026, 6, 30))
        );
        assert_eq!(
            period_containing(CapPeriod::Month, d(2026, 12, 31)),
            (d(2026, 12, 1), d(2026, 12, 31))
        );
        let anchor = d(2026, 6, 1);
        assert_eq!(
            period_containing(CapPeriod::Fortnight { anchor }, date),
            (d(2026, 6, 1), d(2026, 6, 14))
        );
        assert_eq!(
            period_containing(CapPeriod::Fortnight { anchor }, d(2026, 6, 15)),
            (d(2026, 6, 15), d(2026, 6, 28))
        );
        // Dates before the anchor fall in earlier periods.
        assert_eq!(
            period_containing(CapPeriod::Fortnight { anchor }, d(2026, 5, 31)),
            (d(2026, 5, 18), d(2026, 5, 31))
        );
        assert_eq!(
            period_containing(CapPeriod::Custom { anchor, days: 10 }, date),
            (d(2026, 6, 1), d(2026, 6, 10))
        );
    }
}
