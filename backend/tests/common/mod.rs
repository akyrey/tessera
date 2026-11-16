//! Shared harness: a router over a temporary SQLite database.
#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tessera::accounts::model::User;
use tessera::accounts::service::{self, NewCredential};
use tessera::accounts::sessions;
use tessera::clock::now_unix;
use tessera::config::Config;
use tessera::{AppState, db, router};
use tower::ServiceExt;

pub const ORIGIN: &str = "http://localhost:8080";
pub const COOKIE_NAME: &str = "tessera_session";

pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    _dir: TempDir,
}

pub async fn spawn_app() -> TestApp {
    let dir = tempfile::tempdir().expect("temp dir");
    let database_url = format!("sqlite://{}", dir.path().join("test.db").display());
    let config = Config::from_lookup(|key| match key {
        "TESSERA_PUBLIC_URL" => Some(ORIGIN.into()),
        "TESSERA_DATABASE_URL" => Some(database_url.clone()),
        _ => None,
    })
    .expect("valid config");
    let pool = db::connect(&config.database_url).await.expect("database");
    let state = AppState::new(config, pool).expect("state");
    TestApp {
        router: router(state.clone()),
        state,
        _dir: dir,
    }
}

impl TestApp {
    pub async fn send(&self, request: Request<Body>) -> Response {
        self.router.clone().oneshot(request).await.unwrap()
    }

    pub async fn get(&self, uri: &str, cookie: Option<&str>) -> Response {
        let mut builder = Request::get(uri);
        if let Some(token) = cookie {
            builder = builder.header(header::COOKIE, format!("{COOKIE_NAME}={token}"));
        }
        self.send(builder.body(Body::empty()).unwrap()).await
    }

    /// Same-origin POST, as the PWA sends it.
    pub async fn post(&self, uri: &str, body: Value, cookie: Option<&str>) -> Response {
        self.send(json_request("POST", uri, body, cookie, Some(ORIGIN)))
            .await
    }

    pub async fn put(&self, uri: &str, body: Value, cookie: Option<&str>) -> Response {
        self.send(json_request("PUT", uri, body, cookie, Some(ORIGIN)))
            .await
    }

    /// Same-origin PUT of a raw body (photo uploads).
    pub async fn put_bytes(
        &self,
        uri: &str,
        content_type: &str,
        bytes: Vec<u8>,
        cookie: Option<&str>,
    ) -> Response {
        let mut builder = Request::put(uri)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::ORIGIN, ORIGIN);
        if let Some(token) = cookie {
            builder = builder.header(header::COOKIE, format!("{COOKIE_NAME}={token}"));
        }
        self.send(builder.body(Body::from(bytes)).unwrap()).await
    }

    pub async fn delete(&self, uri: &str, cookie: Option<&str>) -> Response {
        self.send(json_request(
            "DELETE",
            uri,
            Value::Null,
            cookie,
            Some(ORIGIN),
        ))
        .await
    }

    /// Creates a user through the real invite flow (with a fake credential)
    /// and returns them with a valid session token.
    pub async fn bootstrap_admin(&self, name: &str) -> (User, String) {
        let invite = service::ensure_bootstrap_invite(&self.state.db, None, now_unix())
            .await
            .unwrap()
            .expect("no users yet");
        self.register(&invite.token.value, name).await
    }

    /// Invites `name` into `owner`'s household and registers them.
    pub async fn add_member(&self, owner: &User, name: &str) -> (User, String) {
        let invite = service::create_household_invite(&self.state.db, owner, now_unix())
            .await
            .unwrap();
        self.register(&invite.token.value, name).await
    }

    pub async fn register(&self, token: &str, name: &str) -> (User, String) {
        let now = now_unix();
        let pending = service::plan_registration(&self.state.db, token, Some(name), now)
            .await
            .unwrap();
        let credential_id = format!("cred-{name}");
        let credential = NewCredential {
            credential_id: credential_id.as_bytes(),
            passkey_json: "{}",
            name: "Test device",
        };
        let user = service::complete_registration(&self.state.db, &pending, &credential, now)
            .await
            .unwrap();
        let session = sessions::start_session(&self.state.db, user.id, 3600, now)
            .await
            .unwrap();
        (user, session.value)
    }
}

pub fn json_request(
    method: &str,
    uri: &str,
    body: Value,
    cookie: Option<&str>,
    origin: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = cookie {
        builder = builder.header(header::COOKIE, format!("{COOKIE_NAME}={token}"));
    }
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    let body = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    builder.body(body).unwrap()
}

pub async fn bytes(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

pub async fn json(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

pub async fn error_code(response: Response) -> String {
    json(response).await["error"]["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}
