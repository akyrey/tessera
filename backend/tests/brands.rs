//! Brand search/linking and store locations, against a fake Wikidata + Overpass.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use common::{TestApp, error_code, json, spawn_app, spawn_app_with_upstream};
use serde_json::{Value, json};
use std::collections::HashMap;
use tessera::brands::{refresh, service};
use tessera::clock::now_unix;
use uuid::Uuid;

#[derive(Clone, Default)]
struct Upstream {
    overpass_calls: Arc<AtomicUsize>,
    overpass_down: bool,
}

async fn wikidata(Query(params): Query<HashMap<String, String>>) -> Json<Value> {
    match params.get("action").map(String::as_str) {
        Some("wbsearchentities") => Json(json!({ "search": [
            { "id": "Q1059636", "label": "Esselunga", "description": "catena di supermercati" },
            { "id": "Q1129493", "label": "Coop Italia" },
        ]})),
        _ => {
            let id = params.get("ids").cloned().unwrap_or_default();
            if id == "Q1059636" {
                Json(
                    json!({ "entities": { "Q1059636": { "labels": { "it": { "value": "Esselunga" } } } } }),
                )
            } else {
                Json(json!({ "entities": { id: { "missing": "" } } }))
            }
        }
    }
}

async fn overpass(State(upstream): State<Upstream>, body: String) -> (StatusCode, Json<Value>) {
    upstream.overpass_calls.fetch_add(1, Ordering::SeqCst);
    assert!(body.contains("brand%3Awikidata"), "{body}");
    if upstream.overpass_down {
        return (StatusCode::GATEWAY_TIMEOUT, Json(json!({})));
    }
    (
        StatusCode::OK,
        Json(json!({ "elements": [
            { "type": "node", "id": 1, "lat": 45.4642, "lon": 9.19, "tags": { "name": "Esselunga Duomo" } },
            { "type": "way", "id": 2, "center": { "lat": 45.48, "lon": 9.2 } },
        ]})),
    )
}

/// Starts the fake upstream and an app wired to it.
async fn spawn(upstream: Upstream) -> TestApp {
    let router = Router::new()
        .route("/w/api.php", get(wikidata))
        .route("/api/interpreter", post(overpass))
        .with_state(upstream);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    spawn_app_with_upstream(&format!("http://{address}/")).await
}

async fn link(app: &TestApp, session: &str, qid: &str) -> axum::response::Response {
    app.post("/api/brands", json!({ "wikidataId": qid }), Some(session))
        .await
}

#[tokio::test]
async fn search_combines_linked_brands_and_wikidata_candidates() {
    let app = spawn(Upstream::default()).await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    link(&app, &session, "Q1059636").await;

    let response = app.get("/api/brands/search?q=Ess", Some(&session)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["linked"][0]["wikidataId"], "Q1059636");
    // The linked brand is not offered again as a candidate.
    let candidates: Vec<_> = body["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["wikidataId"].clone())
        .collect();
    assert_eq!(candidates, [json!("Q1129493")]);
    assert_eq!(body["remoteUnavailable"], false);
}

#[tokio::test]
async fn search_still_answers_when_wikidata_is_down() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let body = json(app.get("/api/brands/search?q=coop", Some(&session)).await).await;

    assert_eq!(body["remoteUnavailable"], true);
    assert_eq!(body["candidates"], json!([]));
}

#[tokio::test]
async fn search_validates_the_query_and_requires_a_session() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let response = app.get("/api/brands/search?q=a", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(response).await, "invalid_request");
    assert_eq!(
        app.get("/api/brands/search?q=coop", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn linking_takes_the_name_from_wikidata_and_is_idempotent() {
    let app = spawn(Upstream::default()).await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let first = link(&app, &session, "Q1059636").await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let brand = json(first).await;
    assert_eq!(brand["name"], "Esselunga");

    let again = link(&app, &session, "Q1059636").await;
    assert_eq!(again.status(), StatusCode::OK);
    assert_eq!(json(again).await["id"], brand["id"]);
}

#[tokio::test]
async fn linking_rejects_bad_or_unknown_items() {
    let app = spawn(Upstream::default()).await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    assert_eq!(
        link(&app, &session, "Q0").await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        link(&app, &session, "P31").await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        link(&app, &session, "Q999").await.status(),
        StatusCode::BAD_REQUEST
    );
}

async fn card_with_brand(app: &TestApp, session: &str, brand_id: &Value) -> Uuid {
    let id = Uuid::now_v7();
    let response = app
        .put(
            &format!("/api/cards/{id}"),
            json!({
                "storeName": "Esselunga", "cardNumber": "123", "barcodeFormat": "none",
                "brandId": brand_id,
                "places": [{ "label": "Shop near home", "lat": 45.1, "lon": 9.1 }],
            }),
            Some(session),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let card = json(response).await;
    assert_eq!(card["brandName"], "Esselunga");
    assert_eq!(card["places"][0]["label"], "Shop near home");
    id
}

#[tokio::test]
async fn locations_reach_users_who_see_a_card_of_the_brand() {
    let upstream = Upstream::default();
    let app = spawn(upstream.clone()).await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let brand = json(link(&app, &alice_session, "Q1059636").await).await;
    let brand_id = Uuid::parse_str(brand["id"].as_str().unwrap()).unwrap();
    card_with_brand(&app, &alice_session, &brand["id"]).await;

    let count = service::refresh_locations(
        &app.state.db,
        &app.state.upstream,
        brand_id,
        "Q1059636",
        now_unix(),
    )
    .await
    .unwrap();
    assert_eq!(count, 2);

    let body = json(app.get("/api/locations", Some(&alice_session)).await).await;
    assert_eq!(body["radiusM"], 200);
    assert_eq!(
        body["brands"][0]["points"],
        json!([[45.4642, 9.19], [45.48, 9.2]])
    );
    let version = body["version"].as_str().unwrap().to_owned();
    let unchanged = json(
        app.get(
            &format!("/api/locations?version={version}"),
            Some(&alice_session),
        )
        .await,
    )
    .await;
    assert_eq!(unchanged["brands"], Value::Null);

    // Bea sees no Esselunga card, so she gets no Esselunga shops.
    assert_eq!(
        json(app.get("/api/locations", Some(&bea_session)).await).await["brands"],
        json!([])
    );
}

#[tokio::test]
async fn cards_validate_brands_and_places() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let uri = format!("/api/cards/{}", Uuid::now_v7());

    for body in [
        json!({ "storeName": "X", "cardNumber": "1", "barcodeFormat": "none", "brandId": Uuid::now_v7() }),
        json!({ "storeName": "X", "cardNumber": "1", "barcodeFormat": "none",
                "places": [{ "label": "x", "lat": 100.0, "lon": 0.0 }] }),
    ] {
        let response = app.put(&uri, body, Some(&session)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn refresh_job_fetches_due_brands_once_and_backs_off_after_errors() {
    let healthy = Upstream::default();
    let app = spawn(healthy.clone()).await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let brand = json(link(&app, &session, "Q1059636").await).await;
    card_with_brand(&app, &session, &brand["id"]).await;
    // `link` already started a fetch in the background; let it finish.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let calls = healthy.overpass_calls.load(Ordering::SeqCst);

    assert_eq!(
        refresh::refresh_due(&app.state, now_unix()).await.unwrap(),
        0
    );
    assert_eq!(healthy.overpass_calls.load(Ordering::SeqCst), calls);

    let week_later = now_unix() + 8 * 24 * 3600;
    assert_eq!(
        refresh::refresh_due(&app.state, week_later).await.unwrap(),
        1
    );

    let down = Upstream {
        overpass_down: true,
        ..Upstream::default()
    };
    let failing = spawn(down.clone()).await;
    let (_, session) = failing.bootstrap_admin("Alice").await;
    let brand = json(link(&failing, &session, "Q1059636").await).await;
    card_with_brand(&failing, &session, &brand["id"]).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let after_first_failure = down.overpass_calls.load(Ordering::SeqCst);
    assert_eq!(
        refresh::refresh_due(&failing.state, now_unix())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        down.overpass_calls.load(Ordering::SeqCst),
        after_first_failure
    );
}
