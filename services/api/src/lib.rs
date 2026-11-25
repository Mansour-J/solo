//! Bookkeeping API — a modular monolith.
//!
//! Layers (dependencies point downwards only):
//! * `api`          HTTP handlers, routing, OpenAPI
//! * `context`, `middleware`  who a request acts for, and cross-cutting HTTP concerns
//! * `services`     business logic and transaction boundaries
//! * `repositories` SQL
//! * `domain`       pure types, rules and calculations
//!
//! See `docs/architecture/overview.md`.

pub mod api;
pub mod config;
pub mod context;
pub mod db;
pub mod domain;
pub mod error;
pub mod middleware;
pub mod migrations;
pub mod repositories;
pub mod services;
pub mod state;

pub use api::app;
pub use config::Config;
pub use state::AppState;
