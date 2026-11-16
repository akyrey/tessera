//! Store brands (Wikidata), their logos (Wikimedia Commons) and their shop
//! locations (OpenStreetMap Overpass).
//!
//! The server only stores where shops are; deciding which ones are near
//! happens on the phone, so the user's location never leaves it.

mod handlers;
pub mod logo;
pub mod model;
pub mod overpass;
pub mod refresh;
pub mod repo;
pub mod service;
pub mod upstream;
pub mod wikidata;

pub use handlers::routes;
