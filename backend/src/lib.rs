//! Tessera: self-hosted loyalty-card wallet.

pub mod config;
pub mod db;
pub mod error;
pub mod http;

pub use http::{AppState, router};
