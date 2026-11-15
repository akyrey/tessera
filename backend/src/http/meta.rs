//! `GET /api/version`: lets the client check it talks to a compatible server.

use axum::Json;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

pub async fn version() -> Json<AppInfo> {
    Json(AppInfo {
        name: env!("CARGO_PKG_NAME").to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}
