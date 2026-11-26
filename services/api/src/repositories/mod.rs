//! SQL access, one module per aggregate. Repositories take a `&mut PgConnection` (usually a
//! transaction from `Db::begin_*`) so services decide transaction boundaries. No business rules here.

pub mod allocations;
pub mod annual;
pub mod audit;
pub mod business_health;
pub mod businesses;
pub mod customers;
pub mod documents;
pub mod expenses;
pub mod home_office;
pub mod insights;
pub mod invoices;
pub mod ledger;
pub mod mileage;
pub mod quotes;
pub mod recurring;
pub mod reports;
pub mod tax_rates;
pub mod users;
