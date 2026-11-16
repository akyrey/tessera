//! Brand logos: Wikidata P154 → Commons thumbnail → stored and served here.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use common::{TestApp, bytes, json, spawn_app_with_upstream};
use serde_json::{Value, json};
use tessera::brands::{refresh, service};
use tessera::clock::now_unix;
use uuid::Uuid;

const QID: &str = "Q1059636";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR-esselunga";

#[derive(Clone)]
struct Fake {
    /// What the thumbnail URL answers with.
    thumbnail: Arc<Mutex<Vec<u8>>>,
}

async fn wikidata(Query(params): Query<HashMap<String, String>>) -> Json<Value> {
    match params.get("action").map(String::as_str) {
        Some("wbgetclaims") if params.get("entity").map(String::as_str) == Some(QID) => {
            Json(json!({ "claims": { "P154": [
                { "rank": "normal", "mainsnak": { "datavalue": { "value": "Esselunga logo.svg" } } }
            ]}}))
        }
        Some("wbgetclaims") => Json(json!({ "claims": {} })),
        _ => Json(json!({ "entities": { QID: { "labels": { "it": { "value": "Esselunga" } } } } })),
    }
}

/// Commons `imageinfo`: points at this same fake server for the thumbnail.
async fn commons(headers: HeaderMap, Query(params): Query<HashMap<String, String>>) -> Json<Value> {
    assert_eq!(params["titles"], "File:Esselunga logo.svg");
    assert_eq!(params["iiurlwidth"], "256");
    let host = headers[header::HOST].to_str().unwrap();
    Json(json!({ "query": { "pages": [{ "imageinfo": [{
        "thumburl": format!("http://{host}/thumb/256px-Esselunga_logo.svg.png")
    }]}]}}))
}

async fn thumbnail(State(fake): State<Fake>) -> impl IntoResponse {
    let data = fake.thumbnail.lock().unwrap().clone();
    ([(header::CONTENT_TYPE, "image/png")], data)
}

async fn spawn(fake: Fake) -> TestApp {
    let router = Router::new()
        .route("/w/api.php", get(wikidata))
        .route("/commons/api.php", get(commons))
        .route("/thumb/256px-Esselunga_logo.svg.png", get(thumbnail))
        // Shops are not under test here: answer with none.
        .route(
            "/api/interpreter",
            post(|| async { Json(json!({ "elements": [] })) }),
        )
        .with_state(fake);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    spawn_app_with_upstream(&format!("http://{address}/")).await
}

fn fake(thumbnail: &[u8]) -> Fake {
    Fake {
        thumbnail: Arc::new(Mutex::new(thumbnail.to_vec())),
    }
}

/// Links the brand and saves a card using it; returns `(brand id, card id)`.
async fn card_with_brand(app: &TestApp, session: &str) -> (Uuid, Uuid) {
    let brand = json(
        app.post("/api/brands", json!({ "wikidataId": QID }), Some(session))
            .await,
    )
    .await;
    let card_id = Uuid::now_v7();
    let response = app
        .put(
            &format!("/api/cards/{card_id}"),
            json!({ "storeName": "Esselunga", "cardNumber": "1", "barcodeFormat": "none",
                    "brandId": brand["id"] }),
            Some(session),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let brand_id = Uuid::parse_str(brand["id"].as_str().unwrap()).unwrap();
    (brand_id, card_id)
}

async fn card(app: &TestApp, session: &str, id: Uuid) -> Value {
    let snapshot = json(app.get("/api/changes?since=0", Some(session)).await).await;
    snapshot["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["id"] == id.to_string())
        .cloned()
        .unwrap()
}

/// The card's logo id once the background fetch has stored it.
async fn wait_for_logo(app: &TestApp, session: &str, id: Uuid) -> String {
    for _ in 0..50 {
        if let Some(logo) = card(app, session, id).await["brandLogoId"].as_str() {
            return logo.to_owned();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("logo never arrived");
}

#[tokio::test]
async fn a_new_brand_gets_its_logo_and_cards_show_it() {
    let app = spawn(fake(PNG)).await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let cursor = json(app.get("/api/changes?since=0", Some(&session)).await).await["cursor"]
        .as_i64()
        .unwrap();
    let (_, card_id) = card_with_brand(&app, &session).await;

    let logo_id = wait_for_logo(&app, &session, card_id).await;

    let response = app
        .get(&format!("/api/brands/logos/{logo_id}"), Some(&session))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "private, max-age=31536000, immutable"
    );
    assert_eq!(bytes(response).await, PNG);
    // Clients learn about the logo through the regular sync.
    let changes = json(
        app.get(&format!("/api/changes?since={cursor}"), Some(&session))
            .await,
    )
    .await;
    assert_eq!(changes["cards"][0]["brandLogoId"], logo_id.as_str());
}

#[tokio::test]
async fn logos_keep_their_id_until_the_image_changes() {
    let fake = fake(PNG);
    let app = spawn(fake.clone()).await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let (brand_id, card_id) = card_with_brand(&app, &session).await;
    let first = wait_for_logo(&app, &session, card_id).await;
    let refresh = || {
        service::refresh_logo(
            &app.state.db,
            &app.state.upstream,
            brand_id,
            QID,
            now_unix(),
        )
    };

    assert!(!refresh().await.unwrap());
    assert_eq!(
        card(&app, &session, card_id).await["brandLogoId"],
        first.as_str()
    );

    *fake.thumbnail.lock().unwrap() = [PNG, b"-v2"].concat();
    assert!(refresh().await.unwrap());
    let second = card(&app, &session, card_id).await["brandLogoId"].clone();
    assert_ne!(second, first.as_str());
    // The old id is gone with the old image.
    let old = app
        .get(&format!("/api/brands/logos/{first}"), Some(&session))
        .await;
    assert_eq!(old.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn anything_but_a_raster_image_is_rejected_and_retried_later() {
    let fake = fake(b"<svg onload=alert(1)></svg>");
    let app = spawn(fake.clone()).await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let (_, card_id) = card_with_brand(&app, &session).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        card(&app, &session, card_id).await["brandLogoId"],
        Value::Null
    );

    // Backing off: the failed brand is not due again right away…
    *fake.thumbnail.lock().unwrap() = PNG.to_vec();
    assert_eq!(
        refresh::refresh_due_logos(&app.state, now_unix())
            .await
            .unwrap(),
        0
    );
    // …but is once the back-off has passed.
    let later = now_unix() + 7 * 60 * 60;
    assert_eq!(
        refresh::refresh_due_logos(&app.state, later).await.unwrap(),
        1
    );
    assert!(card(&app, &session, card_id).await["brandLogoId"].is_string());
}

#[tokio::test]
async fn logos_need_a_session_and_an_existing_id() {
    let app = spawn(fake(PNG)).await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let (_, card_id) = card_with_brand(&app, &session).await;
    let logo_id = wait_for_logo(&app, &session, card_id).await;

    let anonymous = app.get(&format!("/api/brands/logos/{logo_id}"), None).await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let unknown = app
        .get(
            &format!("/api/brands/logos/{}", Uuid::now_v7()),
            Some(&session),
        )
        .await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    let malformed = app.get("/api/brands/logos/nope", Some(&session)).await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
}
