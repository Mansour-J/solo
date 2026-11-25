//! Business logic. Services validate input, enforce rules and permissions, decide transaction
//! boundaries and call repositories. HTTP handlers stay thin and only translate requests/responses.

pub mod accountant_pack;
pub mod allocations;
pub mod annual;
pub mod businesses;
pub mod change_of_use;
pub mod customers;
pub mod dashboard;
pub mod disposals;
pub mod documents;
pub mod email;
pub mod email_settings;
pub mod expenses;
pub mod export;
pub mod gst_returns;
pub mod home_office;
pub mod hooks;
pub mod income;
pub mod income_tax;
pub mod insights;
pub mod invoices;
pub mod key_dates;
pub mod ledger;
pub mod mileage;
pub mod notices_email;
pub mod quotes;
pub mod recurring;
pub mod reminders;
pub mod report_export;
pub mod reports;
pub mod statements;
pub mod storage;
pub mod system_status;
pub mod users;
pub mod xero_export;
