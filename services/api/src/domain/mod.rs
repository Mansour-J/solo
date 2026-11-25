//! Pure domain logic: types, rules and calculations with no I/O. Services call into this module;
//! it never calls services, repositories or HTTP code, so every rule here is unit-testable.

pub mod allocations;
pub mod business;
pub mod change_of_use;
pub mod depreciation;
pub mod documents;
pub mod gst_returns;
pub mod home_office;
pub mod income_tax;
pub mod insights;
pub mod ledger;
pub mod mileage;
pub mod money;
pub mod nz_calendar;
pub mod recurring;
pub mod reminders;
pub mod reports;
pub mod tax;
pub mod user;
pub mod validation;
