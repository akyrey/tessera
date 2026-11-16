//! HTTP tests for cards, preferences and photos.

mod common;

use axum::http::{StatusCode, header};
use common::{TestApp, bytes, error_code, json, spawn_app};
use serde_json::{Value, json};
use tessera::cards::service;
use uuid::Uuid;

const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 0, 0x10, b'J', b'F', b'I', b'F'];

fn card_body() -> Value {
    json!({
        "storeName": "Esselunga",
        "cardNumber": "4006381333931",
        "barcodeFormat": "ean13",
        "memberName": "Alice",
        "color": "#0F766E",
    })
}

fn card_uri(id: Uuid) -> String {
    format!("/api/cards/{id}")
}

async fn create_card(app: &TestApp, session: &str) -> Uuid {
    let id = Uuid::now_v7();
    let response = app.put(&card_uri(id), card_body(), Some(session)).await;
    assert_eq!(response.status(), StatusCode::OK);
    id
}

async fn list(app: &TestApp, session: &str) -> Vec<Value> {
    let response = app.get("/api/cards", Some(session)).await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await.as_array().cloned().unwrap_or_default()
}

async fn set_shared(
    app: &TestApp,
    session: &str,
    card_id: Uuid,
    shared: bool,
) -> axum::response::Response {
    app.put(
        &format!("{}/sharing", card_uri(card_id)),
        json!({ "shared": shared }),
        Some(session),
    )
    .await
}

async fn share_with_household(app: &TestApp, session: &str, card_id: Uuid) {
    let response = set_shared(app, session, card_id, true).await;
    assert_eq!(response.status(), StatusCode::OK);
}

// ---------- create, update, list ----------

#[tokio::test]
async fn cards_require_a_session() {
    let app = spawn_app().await;

    assert_eq!(
        app.get("/api/cards", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let response = app.put(&card_uri(Uuid::now_v7()), card_body(), None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn put_creates_a_card_with_normalised_fields() {
    let app = spawn_app().await;
    let (user, session) = app.bootstrap_admin("Alice").await;
    let id = Uuid::now_v7();
    let body = json!({
        "storeName": "  Esselunga ",
        "cardNumber": "4 006381 333931",
        "barcodeFormat": "ean13",
        "color": "#0F766E",
        "notes": "  ",
    });

    let response = app.put(&card_uri(id), body, Some(&session)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let card = json(response).await;
    assert_eq!(card["id"], id.to_string());
    assert_eq!(card["ownerId"], user.id.to_string());
    assert_eq!(card["storeName"], "Esselunga");
    assert_eq!(card["cardNumber"], "4006381333931");
    assert_eq!(card["color"], "#0f766e");
    assert_eq!(card["notes"], Value::Null);
    assert_eq!(card["isFavorite"], false);
    assert_eq!(card["useCount"], 0);
    assert_eq!(card["frontImageId"], Value::Null);
}

#[tokio::test]
async fn put_is_idempotent_and_updates_existing_cards() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let id = create_card(&app, &session).await;
    let mut changed = card_body();
    changed["storeName"] = json!("Coop");
    changed["barcodeFormat"] = json!("qr");
    changed["cardNumber"] = json!("https://coop.example/c/1");

    app.put(&card_uri(id), changed.clone(), Some(&session))
        .await;
    let response = app.put(&card_uri(id), changed, Some(&session)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let cards = list(&app, &session).await;
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0]["storeName"], "Coop");
    assert_eq!(cards[0]["barcodeFormat"], "qr");
}

#[tokio::test]
async fn put_rejects_invalid_cards_with_a_clear_message() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let mut bad_check_digit = card_body();
    bad_check_digit["cardNumber"] = json!("4006381333932");

    let response = app
        .put(&card_uri(Uuid::now_v7()), bad_check_digit, Some(&session))
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json(response).await;
    assert_eq!(body["error"]["code"], "invalid_request");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("last digit")
    );
}

#[tokio::test]
async fn put_rejects_unknown_formats_fields_and_ids() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let mut unknown_format = card_body();
    unknown_format["barcodeFormat"] = json!("maxicode");
    let mut extra_field = card_body();
    extra_field["ownerId"] = json!(Uuid::now_v7());

    for body in [unknown_format, extra_field] {
        let response = app
            .put(&card_uri(Uuid::now_v7()), body, Some(&session))
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = app
        .put("/api/cards/not-a-uuid", card_body(), Some(&session))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(response).await, "invalid_request");
    let response = app
        .put(&card_uri(Uuid::nil()), card_body(), Some(&session))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_is_sorted_by_store_name_ignoring_case() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    for name in ["coop", "Decathlon", "Auchan"] {
        let mut body = card_body();
        body["storeName"] = json!(name);
        app.put(&card_uri(Uuid::now_v7()), body, Some(&session))
            .await;
    }

    let names: Vec<_> = list(&app, &session)
        .await
        .iter()
        .map(|card| card["storeName"].as_str().unwrap().to_owned())
        .collect();

    assert_eq!(names, ["Auchan", "coop", "Decathlon"]);
}

// ---------- access ----------

#[tokio::test]
async fn private_cards_are_invisible_to_other_users() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let id = create_card(&app, &alice_session).await;

    assert!(list(&app, &bea_session).await.is_empty());
    let mut hijack = card_body();
    hijack["storeName"] = json!("Mine now");
    let response = app.put(&card_uri(id), hijack, Some(&bea_session)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    for response in [
        app.delete(&card_uri(id), Some(&bea_session)).await,
        app.post(
            &format!("{}/use", card_uri(id)),
            Value::Null,
            Some(&bea_session),
        )
        .await,
        app.put(
            &format!("{}/prefs", card_uri(id)),
            json!({"isFavorite": true}),
            Some(&bea_session),
        )
        .await,
    ] {
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    assert_eq!(
        list(&app, &alice_session).await[0]["storeName"],
        "Esselunga"
    );
}

#[tokio::test]
async fn shared_cards_can_be_edited_but_not_deleted_by_members() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let id = create_card(&app, &alice_session).await;
    share_with_household(&app, &alice_session, id).await;

    assert_eq!(list(&app, &bea_session).await.len(), 1);
    let mut edit = card_body();
    edit["memberName"] = json!("Bea");
    assert_eq!(
        app.put(&card_uri(id), edit, Some(&bea_session))
            .await
            .status(),
        StatusCode::OK
    );
    let response = app.delete(&card_uri(id), Some(&bea_session)).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(list(&app, &alice_session).await[0]["memberName"], "Bea");
}

// ---------- delete ----------

#[tokio::test]
async fn delete_hides_the_card_and_is_idempotent() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let id = create_card(&app, &session).await;

    for _ in 0..2 {
        let response = app.delete(&card_uri(id), Some(&session)).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    assert!(list(&app, &session).await.is_empty());
    // A tombstone cannot be brought back by a late, replayed PUT.
    let response = app.put(&card_uri(id), card_body(), Some(&session)).await;
    assert_eq!(response.status(), StatusCode::GONE);
    assert_eq!(error_code(response).await, "card_deleted");
}

#[tokio::test]
async fn delete_erases_the_card_content_and_photos() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let id = create_card(&app, &session).await;
    app.put_bytes(
        &format!("{}/images/front", card_uri(id)),
        "image/jpeg",
        JPEG.to_vec(),
        Some(&session),
    )
    .await;

    app.delete(&card_uri(id), Some(&session)).await;

    let (number, photos): (String, i64) = sqlx::query_as(
        "SELECT card_number, (SELECT COUNT(*) FROM card_images) FROM cards WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!((number.as_str(), photos), ("-", 0));
}

#[tokio::test]
async fn deleting_an_unknown_card_is_not_found() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let response = app.delete(&card_uri(Uuid::now_v7()), Some(&session)).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------- preferences ----------

#[tokio::test]
async fn favourites_and_usage_are_per_user() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let id = create_card(&app, &alice_session).await;
    share_with_household(&app, &alice_session, id).await;

    let response = app
        .put(
            &format!("{}/prefs", card_uri(id)),
            json!({"isFavorite": true}),
            Some(&alice_session),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["isFavorite"], true);
    for _ in 0..2 {
        let response = app
            .post(
                &format!("{}/use", card_uri(id)),
                Value::Null,
                Some(&alice_session),
            )
            .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    let alice_view = &list(&app, &alice_session).await[0];
    assert_eq!(alice_view["isFavorite"], true);
    assert_eq!(alice_view["useCount"], 2);
    assert!(alice_view["lastUsedAt"].is_number());
    let bea_view = &list(&app, &bea_session).await[0];
    assert_eq!(bea_view["isFavorite"], false);
    assert_eq!(bea_view["useCount"], 0);
    assert_eq!(bea_view["lastUsedAt"], Value::Null);
}

// ---------- photos ----------

#[tokio::test]
async fn photos_can_be_uploaded_replaced_downloaded_and_removed() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let id = create_card(&app, &session).await;
    let uri = format!("{}/images/front", card_uri(id));
    // Larger than the 64 KiB JSON limit: the photo route has its own.
    let mut photo = JPEG.to_vec();
    photo.resize(200 * 1024, 7);

    let first = json(
        app.put_bytes(&uri, "image/jpeg", JPEG.to_vec(), Some(&session))
            .await,
    )
    .await;
    let response = app
        .put_bytes(&uri, "image/jpeg", photo.clone(), Some(&session))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let second = json(response).await;

    assert_ne!(first["id"], second["id"]);
    let card = &list(&app, &session).await[0];
    assert_eq!(card["frontImageId"], second["id"]);
    assert_eq!(card["backImageId"], Value::Null);

    let image_uri = format!("/api/images/{}", second["id"].as_str().unwrap());
    let response = app.get(&image_uri, Some(&session)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    assert!(
        response.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    assert_eq!(bytes(response).await, photo);
    let old = format!("/api/images/{}", first["id"].as_str().unwrap());
    assert_eq!(
        app.get(&old, Some(&session)).await.status(),
        StatusCode::NOT_FOUND
    );

    let response = app.delete(&uri, Some(&session)).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(list(&app, &session).await[0]["frontImageId"], Value::Null);
    assert_eq!(
        app.get(&image_uri, Some(&session)).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn photo_uploads_are_validated() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let id = create_card(&app, &session).await;
    let uri = format!("{}/images/front", card_uri(id));
    let mut too_big = JPEG.to_vec();
    too_big.resize(600 * 1024, 0);

    let cases = [
        ("image/svg+xml", b"<svg/>".to_vec(), StatusCode::BAD_REQUEST),
        ("image/png", JPEG.to_vec(), StatusCode::BAD_REQUEST),
        ("image/jpeg", too_big, StatusCode::PAYLOAD_TOO_LARGE),
    ];
    for (content_type, body, status) in cases {
        let response = app
            .put_bytes(&uri, content_type, body, Some(&session))
            .await;
        assert_eq!(response.status(), status, "{content_type}");
    }
    let response = app
        .put_bytes(
            &format!("{}/images/side", card_uri(id)),
            "image/jpeg",
            JPEG.to_vec(),
            Some(&session),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn photos_of_private_cards_are_not_served_to_others() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let id = create_card(&app, &alice_session).await;
    let uri = format!("{}/images/back", card_uri(id));
    let image = json(
        app.put_bytes(&uri, "image/jpeg", JPEG.to_vec(), Some(&alice_session))
            .await,
    )
    .await;
    let image_uri = format!("/api/images/{}", image["id"].as_str().unwrap());

    assert_eq!(
        app.get(&image_uri, Some(&bea_session)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(&image_uri, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let response = app
        .put_bytes(&uri, "image/jpeg", JPEG.to_vec(), Some(&bea_session))
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        app.delete(&uri, Some(&bea_session)).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn card_writes_require_same_origin() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let request = common::json_request(
        "PUT",
        &card_uri(Uuid::now_v7()),
        card_body(),
        Some(&session),
        Some("https://evil.example"),
    );
    let response = app.send(request).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(response).await, "bad_origin");
}

#[tokio::test]
async fn every_write_advances_the_change_sequence() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let seq = || async {
        sqlx::query_scalar::<_, i64>("SELECT value FROM change_counter")
            .fetch_one(&app.state.db)
            .await
            .unwrap()
    };
    let start = seq().await;

    let id = create_card(&app, &session).await;
    app.put(
        &format!("{}/prefs", card_uri(id)),
        json!({"isFavorite": true}),
        Some(&session),
    )
    .await;
    app.delete(&card_uri(id), Some(&session)).await;

    assert_eq!(seq().await, start + 3);
    let card_seq: i64 = sqlx::query_scalar("SELECT change_seq FROM cards WHERE id = ?")
        .bind(id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(card_seq, start + 3);
}

// ---------- quotas ----------

#[tokio::test]
async fn card_count_is_capped_per_user() {
    let app = spawn_app().await;
    let (user, session) = app.bootstrap_admin("Alice").await;
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?)
         INSERT INTO cards (id, owner_id, store_name, card_number, barcode_format,
                            created_at, updated_at, change_seq)
         SELECT randomblob(16), ?, 'Shop', '1', 'none', 0, 0, 0 FROM n",
    )
    .bind(service::MAX_CARDS_PER_USER)
    .bind(user.id)
    .execute(&app.state.db)
    .await
    .unwrap();

    let response = app
        .put(&card_uri(Uuid::now_v7()), card_body(), Some(&session))
        .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(response).await, "quota_exceeded");
}

#[tokio::test]
async fn photo_storage_is_capped_per_owner() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let full = create_card(&app, &session).await;
    let id = create_card(&app, &session).await;
    sqlx::query(
        "INSERT INTO card_images (id, card_id, kind, mime, data, created_at)
         VALUES (randomblob(16), ?, 'front', 'image/jpeg', zeroblob(?), 0)",
    )
    .bind(full)
    .bind(service::MAX_IMAGE_BYTES_PER_USER - 4)
    .execute(&app.state.db)
    .await
    .unwrap();

    let uri = format!("{}/images/front", card_uri(id));
    let response = app
        .put_bytes(&uri, "image/jpeg", JPEG.to_vec(), Some(&session))
        .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(response).await, "quota_exceeded");
    // Replacing the big photo itself is still allowed: its old bytes do not count.
    let replace = format!("{}/images/front", card_uri(full));
    let response = app
        .put_bytes(&replace, "image/jpeg", JPEG.to_vec(), Some(&session))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

// ---------- pull sync ----------

async fn changes(app: &TestApp, session: &str, since: i64) -> Value {
    let response = app
        .get(&format!("/api/changes?since={since}"), Some(session))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

#[tokio::test]
async fn changes_return_a_snapshot_only_when_something_changed() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let id = create_card(&app, &session).await;

    let first = changes(&app, &session, 0).await;
    let cursor = first["cursor"].as_i64().unwrap();
    assert_eq!(first["cards"][0]["id"], id.to_string());

    let unchanged = changes(&app, &session, cursor).await;
    assert_eq!(unchanged, json!({ "cursor": cursor, "cards": null }));

    app.delete(&card_uri(id), Some(&session)).await;
    let after_delete = changes(&app, &session, cursor).await;
    assert!(after_delete["cursor"].as_i64().unwrap() > cursor);
    assert_eq!(after_delete["cards"], json!([]));
}

#[tokio::test]
async fn changes_only_contain_visible_cards() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let private = create_card(&app, &alice_session).await;
    let shared = create_card(&app, &alice_session).await;
    share_with_household(&app, &alice_session, shared).await;

    let snapshot = changes(&app, &bea_session, 0).await;

    let ids: Vec<_> = snapshot["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].clone())
        .collect();
    assert_eq!(ids, [json!(shared.to_string())]);
    assert_ne!(ids, [json!(private.to_string())]);
}

#[tokio::test]
async fn joining_a_household_advances_the_cursor() {
    let app = spawn_app().await;
    let (alice, session) = app.bootstrap_admin("Alice").await;
    let cursor = changes(&app, &session, 0).await["cursor"].as_i64().unwrap();

    app.add_member(&alice, "Bea").await;

    assert!(
        changes(&app, &session, cursor).await["cursor"]
            .as_i64()
            .unwrap()
            > cursor
    );
}

#[tokio::test]
async fn changes_validate_the_cursor_and_require_a_session() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let response = app.get("/api/changes?since=abc", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(response).await, "invalid_request");
    assert_eq!(
        app.get("/api/changes", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// ---------- sharing ----------

#[tokio::test]
async fn sharing_shows_the_card_to_the_household_and_unsharing_hides_it() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let id = create_card(&app, &alice_session).await;
    assert_eq!(list(&app, &alice_session).await[0]["shared"], false);

    let response = set_shared(&app, &alice_session, id, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["shared"], true);
    let bea_view = &list(&app, &bea_session).await[0];
    assert_eq!(bea_view["ownerName"], "Alice");
    assert_eq!(bea_view["shared"], true);

    let cursor = changes(&app, &bea_session, 0).await["cursor"]
        .as_i64()
        .unwrap();
    set_shared(&app, &alice_session, id, false).await;
    assert_eq!(
        changes(&app, &bea_session, cursor).await["cards"],
        json!([])
    );
}

#[tokio::test]
async fn only_the_owner_can_change_sharing() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let shared = create_card(&app, &alice_session).await;
    share_with_household(&app, &alice_session, shared).await;
    let private = create_card(&app, &alice_session).await;

    let response = set_shared(&app, &bea_session, shared, false).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = set_shared(&app, &bea_session, private, true).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    app.delete(&card_uri(private), Some(&alice_session)).await;
    let response = set_shared(&app, &alice_session, private, true).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn photos_of_shared_cards_are_visible_to_the_household() {
    let app = spawn_app().await;
    let (alice, alice_session) = app.bootstrap_admin("Alice").await;
    let (_, bea_session) = app.add_member(&alice, "Bea").await;
    let id = create_card(&app, &alice_session).await;
    share_with_household(&app, &alice_session, id).await;

    // A member adds the photo; the owner and the member can both see it.
    let uri = format!("{}/images/front", card_uri(id));
    let image = json(
        app.put_bytes(&uri, "image/jpeg", JPEG.to_vec(), Some(&bea_session))
            .await,
    )
    .await;
    let image_uri = format!("/api/images/{}", image["id"].as_str().unwrap());

    assert_eq!(
        app.get(&image_uri, Some(&alice_session)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        app.get(&image_uri, Some(&bea_session)).await.status(),
        StatusCode::OK
    );
}
