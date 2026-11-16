//! Loyalty cards: validation, access policy, storage and photos.

pub mod barcode;
mod handlers;
pub mod images;
pub mod model;
pub mod policy;
pub mod repo;
pub mod service;

pub use handlers::routes;
