use std::io::{Cursor, Write};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use http_body_util::BodyExt;
use nortex_marketplace::{router, validate_package, AppState, Config};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const PW: &str = "correct horse battery";
const SEED: &str = "0101010101010101010101010101010101010101010101010101010101010101";

fn make(name: &str, key: Option<&str>, max: usize) -> (AppState, Router) {
    let path =
        std::env::temp_dir().join(format!("nortex-mkt-{name}-{}.sqlite3", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let cfg = Config {
        bind: "127.0.0.1:0".into(),
        db_path: path.to_string_lossy().into(),
        max_upload_bytes: max,
        token_ttl_secs: 3600,
        signing_key_hex: key.map(String::from),
    };
    let s = AppState::new(cfg).unwrap();
    (s.clone(), router(s))
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Vec<u8>,
    ct: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut r = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", ct);
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    let resp = app
        .clone()
        .oneshot(r.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let (p, b) = resp.into_parts();
    (
        p.status,
        p.headers,
        b.collect().await.unwrap().to_bytes().to_vec(),
    )
}

async fn jcall(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    v: Value,
) -> (StatusCode, Value) {
    let (s, _, b) = call(
        app,
        method,
        uri,
        token,
        v.to_string().into_bytes(),
        "application/json",
    )
    .await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn login(app: &Router, user: &str) -> String {
    let (s, v) = jcall(
        app,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({"username": user, "password": PW}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    v["token"].as_str().unwrap().to_string()
}

fn pkg(files: &[(&str, &str)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, c) in files {
        w.start_file(*n, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(c.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn manifest(id: &str, ver: &str) -> String {
    json!({
        "id": id,
        "name": "Test Ext",
        "version": ver,
        "description": "hello world",
        "entry": "main.nx",
        "nortex": "*",
        "dependencies": {},
        "permissions": []
    })
    .to_string()
}

fn good(id: &str, ver: &str) -> Vec<u8> {
    pkg(&[
        ("nortex-extension.json", &manifest(id, ver)),
        ("main.nx", "// code"),
    ])
}

#[test]
fn package_manifest_validates_permissions_and_version_ranges() {
    let invalid_permission = json!({
        "id": "test.ext",
        "name": "Test",
        "version": "1.0.0",
        "permissions": ["filesystem.everywhere"]
    })
    .to_string();
    let invalid_permission_package = pkg(&[
        ("nortex-extension.json", &invalid_permission),
        ("main.nx", ""),
    ]);
    assert!(validate_package(&invalid_permission_package)
        .unwrap_err()
        .contains("permission"));

    let invalid_dependency = json!({
        "id": "test.ext",
        "name": "Test",
        "version": "1.0.0",
        "dependencies": {"other.ext": "not-semver"}
    })
    .to_string();
    let invalid_dependency_package = pkg(&[
        ("nortex-extension.json", &invalid_dependency),
        ("main.nx", ""),
    ]);
    assert!(validate_package(&invalid_dependency_package)
        .unwrap_err()
        .contains("dependency"));
}

#[tokio::test]
async fn full_flow() {
    let (state, app) = make("flow", Some(SEED), 1024 * 1024);
    let (s, _) = jcall(&app, "GET", "/api/v1/health", None, Value::Null).await;
    assert_eq!(s, StatusCode::OK);

    state.create_user("root", PW, "admin").unwrap();
    for u in ["pub1", "alice"] {
        let (s, _) = jcall(
            &app,
            "POST",
            "/api/v1/auth/register",
            None,
            json!({"username": u, "password": PW}),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
    }
    let (s, _) = jcall(
        &app,
        "POST",
        "/api/v1/auth/register",
        None,
        json!({"username": "alice", "password": PW}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = jcall(
        &app,
        "POST",
        "/api/v1/auth/register",
        None,
        json!({"username": "bob1", "password": "short"}),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = jcall(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({"username": "alice", "password": "wrong-password"}),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let admin = login(&app, "root").await;
    let publisher = login(&app, "pub1").await;
    let alice = login(&app, "alice").await;

    // authz: plain user cannot upload; publisher role needs admin grant; non-admin cannot grant
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/extensions",
        Some(&publisher),
        good("demo.ext", "1.0.0"),
        "application/zip",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = jcall(
        &app,
        "PUT",
        "/api/v1/admin/users/pub1/role",
        Some(&alice),
        json!({"role": "publisher"}),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = jcall(
        &app,
        "PUT",
        "/api/v1/admin/users/pub1/role",
        Some(&admin),
        json!({"role": "publisher"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/extensions",
        None,
        good("demo.ext", "1.0.0"),
        "application/zip",
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let data = good("demo.ext", "1.0.0");
    let (s, _, b) = call(
        &app,
        "POST",
        "/api/v1/extensions",
        Some(&publisher),
        data.clone(),
        "application/zip",
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let sub: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(sub["status"], "pending");
    let sid = sub["submission_id"].as_i64().unwrap();
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/extensions",
        Some(&publisher),
        data.clone(),
        "application/zip",
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);

    // pending is invisible and not downloadable
    let (_, v) = jcall(&app, "GET", "/api/v1/extensions?q=demo", None, Value::Null).await;
    assert_eq!(v["results"].as_array().unwrap().len(), 0);
    let (s, _, _) = call(
        &app,
        "GET",
        "/api/v1/extensions/demo.ext/versions/1.0.0/download",
        None,
        vec![],
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, _) = jcall(
        &app,
        "POST",
        &format!("/api/v1/admin/submissions/{sid}/approve"),
        Some(&publisher),
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, v) = jcall(
        &app,
        "GET",
        "/api/v1/admin/submissions",
        Some(&admin),
        Value::Null,
    )
    .await;
    assert_eq!(v["submissions"].as_array().unwrap().len(), 1);
    let manifest: Value =
        serde_json::from_str(v["submissions"][0]["manifest"].as_str().unwrap()).unwrap();
    assert_eq!(manifest["id"], "demo.ext");
    assert!(manifest["permissions"].is_array());
    let (s, _) = jcall(
        &app,
        "POST",
        &format!("/api/v1/admin/submissions/{sid}/approve"),
        Some(&admin),
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (_, v) = jcall(&app, "GET", "/api/v1/extensions?q=demo", None, Value::Null).await;
    assert_eq!(v["results"][0]["latest_version"], "1.0.0");
    let (s, v) = jcall(
        &app,
        "GET",
        "/api/v1/extensions/demo.ext",
        None,
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["publisher"], "pub1");

    // download: hash + signature verify independently
    let (s, h, body) = call(
        &app,
        "GET",
        "/api/v1/extensions/demo.ext/versions/1.0.0/download",
        None,
        vec![],
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, data);
    let digest = Sha256::digest(&body);
    assert_eq!(h["x-nortex-sha256"].to_str().unwrap(), hex::encode(digest));
    let (_, k) = jcall(&app, "GET", "/api/v1/signing-key", None, Value::Null).await;
    let pk: [u8; 32] = hex::decode(k["public_key_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let sig =
        Signature::from_slice(&hex::decode(h["x-nortex-signature"].to_str().unwrap()).unwrap())
            .unwrap();
    VerifyingKey::from_bytes(&pk)
        .unwrap()
        .verify(&digest, &sig)
        .unwrap();

    // ratings & favorites
    let (s, _) = jcall(
        &app,
        "PUT",
        "/api/v1/extensions/demo.ext/rating",
        None,
        json!({"stars": 4}),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = jcall(
        &app,
        "PUT",
        "/api/v1/extensions/demo.ext/rating",
        Some(&alice),
        json!({"stars": 9}),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = jcall(
        &app,
        "PUT",
        "/api/v1/extensions/demo.ext/rating",
        Some(&alice),
        json!({"stars": 4}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, v) = jcall(
        &app,
        "GET",
        "/api/v1/extensions/demo.ext",
        None,
        Value::Null,
    )
    .await;
    assert_eq!(v["rating_count"], 1);
    assert_eq!(v["rating_average"], 4.0);
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/extensions/demo.ext/favorite",
        Some(&alice),
        vec![],
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, v) = jcall(
        &app,
        "GET",
        "/api/v1/me/favorites",
        Some(&alice),
        Value::Null,
    )
    .await;
    assert_eq!(v["favorites"].as_array().unwrap().len(), 1);
    let (s, _, _) = call(
        &app,
        "DELETE",
        "/api/v1/extensions/demo.ext/favorite",
        Some(&alice),
        vec![],
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);

    // second version by another publisher is refused; same publisher -> update check
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/extensions",
        Some(&publisher),
        good("demo.ext", "1.2.0"),
        "application/zip",
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let (_, v) = jcall(
        &app,
        "POST",
        "/api/v1/updates/check",
        None,
        json!({"installed": [{"id": "demo.ext", "version": "1.0.0"}]}),
    )
    .await;
    assert_eq!(v["updates"].as_array().unwrap().len(), 0);
    let (_, v) = jcall(
        &app,
        "POST",
        &format!("/api/v1/admin/submissions/{}/approve", sid + 1),
        Some(&admin),
        Value::Null,
    )
    .await;
    assert_eq!(v["status"], "published");
    let (_, v) = jcall(&app, "POST", "/api/v1/updates/check", None, json!({"installed": [{"id": "demo.ext", "version": "1.0.0"}, {"id": "demo.ext", "version": "1.2.0"}]})).await;
    assert_eq!(v["updates"].as_array().unwrap().len(), 1);
    assert_eq!(v["updates"][0]["latest_version"], "1.2.0");

    // logout revokes token
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/auth/logout",
        Some(&alice),
        vec![],
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = jcall(
        &app,
        "GET",
        "/api/v1/me/favorites",
        Some(&alice),
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/extensions/demo.ext/run",
        None,
        vec![],
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn rejects_bad_packages_and_limits() {
    let (state, app) = make("bad", Some(SEED), 4096);
    state.create_user("root", PW, "admin").unwrap();
    let t = login(&app, "root").await;
    let up = |d: Vec<u8>| {
        let app = app.clone();
        let t = t.clone();
        async move {
            call(
                &app,
                "POST",
                "/api/v1/extensions",
                Some(&t),
                d,
                "application/zip",
            )
            .await
            .0
        }
    };
    let m = manifest("bad.ext", "1.0.0");
    assert_eq!(up(b"not a zip".to_vec()).await, StatusCode::BAD_REQUEST);
    assert_eq!(
        up(pkg(&[("nortex-extension.json", &m), ("../evil.txt", "x")])).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        up(pkg(&[("nortex-extension.json", &m), ("/abs.txt", "x")])).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        up(pkg(&[("nortex-extension.json", &m), ("a\\b.txt", "x")])).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(up(pkg(&[("main.nx", "x")])).await, StatusCode::BAD_REQUEST);
    assert_eq!(
        up(pkg(&[
            ("nortex-extension.json", &manifest("bad.ext", "1.0")),
            ("main.nx", "x")
        ]))
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        up(pkg(&[
            ("nortex-extension.json", &manifest("Bad Id", "1.0.0")),
            ("main.nx", "x")
        ]))
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        up(pkg(&[("nortex-extension.json", &m)])).await,
        StatusCode::BAD_REQUEST
    ); // entry missing
    assert_eq!(up(vec![0u8; 10_000]).await, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn unsigned_server_returns_503() {
    let (state, app) = make("nosign", None, 1024 * 1024);
    state.create_user("root", PW, "admin").unwrap();
    let t = login(&app, "root").await;
    let (s, _, b) = call(
        &app,
        "POST",
        "/api/v1/extensions",
        Some(&t),
        good("x.ext", "1.0.0"),
        "application/zip",
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let sid = serde_json::from_slice::<Value>(&b).unwrap()["submission_id"]
        .as_i64()
        .unwrap();
    let (s, _) = jcall(
        &app,
        "POST",
        &format!("/api/v1/admin/submissions/{sid}/approve"),
        Some(&t),
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
    let (s, _, _) = call(
        &app,
        "GET",
        "/api/v1/extensions/x.ext/versions/1.0.0/download",
        None,
        vec![],
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
}
