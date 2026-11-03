//! NORTEX marketplace backend. Uploaded archives are only inspected, never extracted or executed.

use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub mod nxpack;

pub const MANIFEST_NAME: &str = "nortex-extension.json";
const MAX_ARCHIVE_ENTRIES: usize = 5000;
const MAX_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_UPDATE_ITEMS: usize = 500;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub db_path: String,
    pub max_upload_bytes: usize,
    pub token_ttl_secs: u64,
    /// 32-byte Ed25519 seed as hex; supplied only via environment.
    pub signing_key_hex: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Config, String> {
        fn num<T: std::str::FromStr>(name: &str, default: T) -> Result<T, String> {
            match std::env::var(name) {
                Ok(v) => v
                    .trim()
                    .parse()
                    .map_err(|_| format!("{name} is not a valid number")),
                Err(_) => Ok(default),
            }
        }
        Ok(Config {
            bind: std::env::var("NORTEX_MARKET_BIND").unwrap_or_else(|_| "127.0.0.1:8787".into()),
            db_path: std::env::var("NORTEX_MARKET_DB")
                .unwrap_or_else(|_| "nortex-marketplace.sqlite3".into()),
            max_upload_bytes: num("NORTEX_MARKET_MAX_UPLOAD_BYTES", 10 * 1024 * 1024)?,
            token_ttl_secs: num("NORTEX_MARKET_TOKEN_TTL_SECS", 24 * 3600)?,
            signing_key_hex: std::env::var("NORTEX_MARKET_SIGNING_KEY_HEX")
                .ok()
                .filter(|s| !s.trim().is_empty()),
        })
    }
}

#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl ApiError {
    fn new(code: StatusCode, msg: impl Into<String>) -> Self {
        ApiError(code, msg.into())
    }
    pub fn message(&self) -> &str {
        &self.1
    }
    fn bad(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }
    fn internal<E: std::fmt::Display>(e: E) -> Self {
        eprintln!("internal error: {e}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

struct Inner {
    db: Mutex<Connection>,
    cfg: Config,
    signing: Option<SigningKey>,
    dummy_hash: String,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

struct User {
    id: i64,
    role: String,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hash_password(pw: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(ApiError::internal)
}

fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|p| Argon2::default().verify_password(pw.as_bytes(), &p).is_ok())
        .unwrap_or(false)
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn valid_username(u: &str) -> bool {
    (3..=32).contains(&u.len())
        && u.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn valid_extension_id(id: &str) -> bool {
    let b = id.as_bytes();
    (3..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_')
        })
        && !id.contains("..")
}

/// Rejects absolute paths, traversal, backslashes, drive letters, NUL and control characters.
pub fn safe_archive_path(name: &str) -> bool {
    if name.is_empty()
        || name.len() > 255
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
    {
        return false;
    }
    if name.chars().any(|c| c.is_control()) {
        return false;
    }
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    !trimmed.is_empty()
        && trimmed
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

impl AppState {
    pub fn new(cfg: Config) -> Result<AppState, String> {
        let signing = match &cfg.signing_key_hex {
            Some(h) => {
                let bytes =
                    hex::decode(h.trim()).map_err(|_| "signing key must be hex".to_string())?;
                let seed: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| "signing key must be 32 bytes (64 hex chars)".to_string())?;
                Some(SigningKey::from_bytes(&seed))
            }
            None => None,
        };
        let conn = Connection::open(&cfg.db_path).map_err(|e| e.to_string())?;
        conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        let dummy_hash = hash_password("dummy-password-for-timing").map_err(|e| e.1)?;
        Ok(AppState(Arc::new(Inner {
            db: Mutex::new(conn),
            cfg,
            signing,
            dummy_hash,
        })))
    }

    pub fn signing_configured(&self) -> bool {
        self.0.signing.is_some()
    }

    pub fn create_user(&self, username: &str, password: &str, role: &str) -> ApiResult<()> {
        if !valid_username(username) {
            return Err(ApiError::bad(
                "username must be 3-32 chars of letters, digits, '_' or '-'",
            ));
        }
        if !(10..=256).contains(&password.len()) {
            return Err(ApiError::bad("password must be 10-256 bytes"));
        }
        let hash = hash_password(password)?;
        let db = self.0.db.lock().unwrap();
        match db.execute(
            "INSERT INTO users(username, password_hash, role, created_at) VALUES(?1, ?2, ?3, ?4)",
            params![username, hash, role, now()],
        ) {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "username already taken",
                ))
            }
            Err(e) => Err(e.into()),
        }
    }

    fn authenticate(&self, headers: &HeaderMap) -> ApiResult<User> {
        let unauthorized =
            || ApiError::new(StatusCode::UNAUTHORIZED, "missing or invalid bearer token");
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(unauthorized)?;
        let db = self.0.db.lock().unwrap();
        db.query_row(
            "SELECT u.id, u.role FROM tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = ?1 AND t.expires_at > ?2",
            params![sha256_hex(token.as_bytes()), now()],
            |r| Ok(User { id: r.get(0)?, role: r.get(1)? }),
        )
        .optional()?
        .ok_or_else(unauthorized)
    }

    fn require_role(&self, headers: &HeaderMap, roles: &[&str]) -> ApiResult<User> {
        let u = self.authenticate(headers)?;
        if roles.contains(&u.role.as_str()) {
            Ok(u)
        } else {
            Err(ApiError::new(StatusCode::FORBIDDEN, "insufficient role"))
        }
    }

    fn signer(&self) -> ApiResult<&SigningKey> {
        self.0.signing.as_ref().ok_or_else(|| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "package signing key is not configured",
            )
        })
    }
}

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS users(
  id INTEGER PRIMARY KEY, username TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('user','publisher','admin')), created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS tokens(
  token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE, expires_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS extensions(
  id TEXT PRIMARY KEY, publisher_id INTEGER NOT NULL REFERENCES users(id), name TEXT NOT NULL,
  description TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS versions(
  id INTEGER PRIMARY KEY, ext_id TEXT NOT NULL REFERENCES extensions(id), version TEXT NOT NULL,
  manifest TEXT NOT NULL, sha256 TEXT NOT NULL, size INTEGER NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('pending','published','rejected')),
  signature TEXT, package BLOB NOT NULL, uploaded_at INTEGER NOT NULL, published_at INTEGER,
  UNIQUE(ext_id, version));
CREATE TABLE IF NOT EXISTS ratings(
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE, ext_id TEXT NOT NULL REFERENCES extensions(id),
  stars INTEGER NOT NULL CHECK(stars BETWEEN 1 AND 5), updated_at INTEGER NOT NULL, PRIMARY KEY(user_id, ext_id));
CREATE TABLE IF NOT EXISTS favorites(
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE, ext_id TEXT NOT NULL REFERENCES extensions(id),
  PRIMARY KEY(user_id, ext_id));
";

pub fn router(state: AppState) -> Router {
    let limit = state.0.cfg.max_upload_bytes;
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/signing-key", get(signing_key))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/extensions", get(search).post(upload))
        .route("/api/v1/extensions/:id", get(details))
        .route(
            "/api/v1/extensions/:id/versions/:version/download",
            get(download),
        )
        .route("/api/v1/extensions/:id/rating", put(rate))
        .route(
            "/api/v1/extensions/:id/favorite",
            put(favorite_add).delete(favorite_remove),
        )
        .route("/api/v1/extensions/:id/run", post(unsupported_runtime))
        .route("/api/v1/me/favorites", get(favorites_list))
        .route("/api/v1/updates/check", post(updates_check))
        .route("/api/v1/admin/submissions", get(submissions))
        .route("/api/v1/admin/submissions/:id/approve", post(approve))
        .route("/api/v1/admin/submissions/:id/reject", post(reject))
        .route("/api/v1/admin/users/:username/role", put(set_role))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(state)
}

async fn health(State(s): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "signing_configured": s.signing_configured() }))
}

async fn signing_key(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let k = s.signer()?;
    Ok(Json(
        json!({ "algorithm": "ed25519", "public_key_hex": hex::encode(k.verifying_key().to_bytes()), "signed_message": "raw 32-byte SHA-256 digest of the package" }),
    ))
}

async fn unsupported_runtime() -> ApiError {
    ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "the marketplace never executes extensions; runtime hosting is not implemented",
    )
}

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
}

async fn register(
    State(s): State<AppState>,
    Json(c): Json<Credentials>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let s2 = s.clone();
    tokio::task::spawn_blocking(move || s2.create_user(&c.username, &c.password, "user"))
        .await
        .map_err(ApiError::internal)??;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "status": "registered", "role": "user" })),
    ))
}

async fn login(State(s): State<AppState>, Json(c): Json<Credentials>) -> ApiResult<Json<Value>> {
    let s2 = s.clone();
    tokio::task::spawn_blocking(move || -> ApiResult<Json<Value>> {
        let row = {
            let db = s2.0.db.lock().unwrap();
            db.query_row(
                "SELECT id, password_hash, role FROM users WHERE username = ?1",
                params![c.username],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
        };
        let bad = || ApiError::new(StatusCode::UNAUTHORIZED, "invalid username or password");
        let Some((uid, hash, role)) = row else {
            verify_password(&c.password, &s2.0.dummy_hash);
            return Err(bad());
        };
        if !verify_password(&c.password, &hash) {
            return Err(bad());
        }
        let mut raw = [0u8; 32];
        OsRng.fill_bytes(&mut raw);
        let token = hex::encode(raw);
        let exp = now() + s2.0.cfg.token_ttl_secs as i64;
        let db = s2.0.db.lock().unwrap();
        db.execute("DELETE FROM tokens WHERE expires_at <= ?1", params![now()])?;
        db.execute(
            "INSERT INTO tokens(token_hash, user_id, expires_at) VALUES(?1, ?2, ?3)",
            params![sha256_hex(token.as_bytes()), uid, exp],
        )?;
        Ok(Json(
            json!({ "token": token, "token_type": "Bearer", "expires_at": exp, "role": role }),
        ))
    })
    .await
    .map_err(ApiError::internal)?
}

async fn logout(State(s): State<AppState>, headers: HeaderMap) -> ApiResult<StatusCode> {
    s.authenticate(&headers)?;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    s.0.db.lock().unwrap().execute(
        "DELETE FROM tokens WHERE token_hash = ?1",
        params![sha256_hex(token.as_bytes())],
    )?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct RoleBody {
    role: String,
}

async fn set_role(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
    Json(b): Json<RoleBody>,
) -> ApiResult<Json<Value>> {
    s.require_role(&headers, &["admin"])?;
    if !["user", "publisher", "admin"].contains(&b.role.as_str()) {
        return Err(ApiError::bad("role must be user, publisher or admin"));
    }
    let n = s.0.db.lock().unwrap().execute(
        "UPDATE users SET role = ?1 WHERE username = ?2",
        params![b.role, username],
    )?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "user not found"));
    }
    Ok(Json(json!({ "username": username, "role": b.role })))
}

#[derive(Deserialize)]
struct Manifest {
    id: String,
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    nortex: Option<String>,
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    permissions: Vec<String>,
}

/// Validates the archive structure and manifest without extracting anything.
pub fn validate_package(data: &[u8]) -> Result<(Value, String, String), String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(data))
        .map_err(|_| "package is not a valid zip archive".to_string())?;
    if zip.is_empty() || zip.len() > MAX_ARCHIVE_ENTRIES {
        return Err(format!(
            "archive must contain 1-{MAX_ARCHIVE_ENTRIES} entries"
        ));
    }
    let mut total: u64 = 0;
    let mut names = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let f = zip
            .by_index_raw(i)
            .map_err(|_| "unreadable archive entry".to_string())?;
        if !safe_archive_path(f.name()) {
            return Err(format!("unsafe archive path: {:?}", f.name()));
        }
        if matches!(f.unix_mode(), Some(m) if m & 0o170000 == 0o120000) {
            return Err("symbolic links are not allowed in packages".into());
        }
        total = total.saturating_add(f.size());
        if total > MAX_UNCOMPRESSED_BYTES {
            return Err("archive declares too much uncompressed data".into());
        }
        if !names.insert(f.name().to_string()) {
            return Err("duplicate archive entry".into());
        }
    }
    let mut mf = zip
        .by_name(MANIFEST_NAME)
        .map_err(|_| format!("archive must contain {MANIFEST_NAME} at its root"))?;
    if mf.size() > MAX_MANIFEST_BYTES {
        return Err("manifest too large".into());
    }
    let mut buf = Vec::new();
    mf.by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|_| "cannot read manifest".to_string())?;
    if buf.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("manifest too large".into());
    }
    drop(mf);
    let raw: Value =
        serde_json::from_slice(&buf).map_err(|e| format!("manifest is not valid JSON: {e}"))?;
    let m: Manifest =
        serde_json::from_value(raw.clone()).map_err(|e| format!("invalid manifest: {e}"))?;
    if !valid_extension_id(&m.id) {
        return Err("manifest id must be 3-64 chars of [a-z0-9._-], starting alphanumeric".into());
    }
    if m.name.trim().is_empty() || m.name.chars().count() > 100 {
        return Err("manifest name must be 1-100 characters".into());
    }
    if m.description.chars().count() > 2000 {
        return Err("manifest description too long".into());
    }
    let v = semver::Version::parse(&m.version)
        .map_err(|_| "manifest version must be valid semver".to_string())?;
    if v.to_string() != m.version || v.build != semver::BuildMetadata::EMPTY {
        return Err("manifest version must be canonical semver without build metadata".into());
    }
    if let Some(requirement) = &m.nortex {
        if requirement.len() > 256 || semver::VersionReq::parse(requirement).is_err() {
            return Err("manifest NORTEX compatibility range is invalid".into());
        }
    }
    if m.dependencies.len() > 128 {
        return Err("manifest contains too many dependencies".into());
    }
    for (id, requirement) in &m.dependencies {
        if !valid_extension_id(id)
            || id == &m.id
            || requirement.len() > 256
            || semver::VersionReq::parse(requirement).is_err()
        {
            return Err(format!("manifest dependency '{id}' is invalid"));
        }
    }
    if m.permissions.len() > 16
        || m.permissions.iter().any(|permission| {
            !matches!(
                permission.as_str(),
                "network.client"
                    | "network.private"
                    | "filesystem.package.read"
                    | "filesystem.package.write"
            )
        })
    {
        return Err("manifest contains an unsupported permission".into());
    }
    if let Some(e) = &m.entry {
        if !safe_archive_path(e) || e.ends_with('/') || zip.by_name(e).is_err() {
            return Err("manifest entry must be a safe path of a file inside the archive".into());
        }
    }
    Ok((raw, m.id, m.version))
}

async fn upload(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let user = s.require_role(&headers, &["publisher", "admin"])?;
    let (manifest, id, version) = validate_package(&body).map_err(ApiError::bad)?;
    let m: Manifest = serde_json::from_value(manifest.clone()).map_err(ApiError::internal)?;
    let digest = sha256_hex(&body);
    let db = s.0.db.lock().unwrap();
    let owner: Option<i64> = db
        .query_row(
            "SELECT publisher_id FROM extensions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    match owner {
        Some(o) if o != user.id && user.role != "admin" => {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "extension id belongs to another publisher",
            ));
        }
        Some(_) => {}
        None => {
            db.execute(
                "INSERT INTO extensions(id, publisher_id, name, description, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
                params![id, user.id, m.name, m.description, now()],
            )?;
        }
    }
    let r = db.execute(
        "INSERT INTO versions(ext_id, version, manifest, sha256, size, status, package, uploaded_at) VALUES(?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7)",
        params![id, version, manifest.to_string(), digest, body.len() as i64, body.to_vec(), now()],
    );
    match r {
        Ok(_) => {
            let vid = db.last_insert_rowid();
            Ok((
                StatusCode::ACCEPTED,
                Json(
                    json!({ "submission_id": vid, "id": id, "version": version, "status": "pending", "sha256": digest }),
                ),
            ))
        }
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Err(ApiError::new(
                StatusCode::CONFLICT,
                "this version already exists",
            ))
        }
        Err(e) => Err(e.into()),
    }
}

async fn submissions(State(s): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    s.require_role(&headers, &["admin"])?;
    let db = s.0.db.lock().unwrap();
    let mut st = db.prepare(
        "SELECT v.id, v.ext_id, v.version, v.size, v.sha256, u.username, v.manifest FROM versions v JOIN extensions e ON e.id = v.ext_id \
         JOIN users u ON u.id = e.publisher_id WHERE v.status = 'pending' ORDER BY v.id LIMIT 200",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(json!({ "submission_id": r.get::<_, i64>(0)?, "id": r.get::<_, String>(1)?, "version": r.get::<_, String>(2)?,
                       "size": r.get::<_, i64>(3)?, "sha256": r.get::<_, String>(4)?, "publisher": r.get::<_, String>(5)?,
                       "manifest": r.get::<_, String>(6)? }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({ "submissions": rows })))
}

async fn approve(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(sid): Path<i64>,
) -> ApiResult<Json<Value>> {
    s.require_role(&headers, &["admin"])?;
    let key = s.signer()?;
    let db = s.0.db.lock().unwrap();
    let row: Option<(String, String, Vec<u8>, String)> = db
        .query_row(
            "SELECT status, sha256, package, ext_id FROM versions WHERE id = ?1",
            params![sid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((status, digest, pkg, ext)) = row else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "submission not found"));
    };
    if status != "pending" {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "submission is not pending",
        ));
    }
    let raw = Sha256::digest(&pkg);
    if hex::encode(raw) != digest {
        return Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stored package failed integrity check",
        ));
    }
    let sig = hex::encode(key.sign(&raw).to_bytes());
    db.execute(
        "UPDATE versions SET status='published', signature=?1, published_at=?2 WHERE id=?3",
        params![sig, now(), sid],
    )?;
    Ok(Json(
        json!({ "submission_id": sid, "id": ext, "status": "published", "sha256": digest, "signature": sig }),
    ))
}

async fn reject(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(sid): Path<i64>,
) -> ApiResult<Json<Value>> {
    s.require_role(&headers, &["admin"])?;
    let n = s.0.db.lock().unwrap().execute(
        "UPDATE versions SET status='rejected' WHERE id=?1 AND status='pending'",
        params![sid],
    )?;
    if n == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "no pending submission with that id",
        ));
    }
    Ok(Json(json!({ "submission_id": sid, "status": "rejected" })))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

fn latest_published(
    db: &Connection,
    ext: &str,
) -> rusqlite::Result<Option<(semver::Version, String)>> {
    let mut st = db.prepare(
        "SELECT version, sha256 FROM versions WHERE ext_id = ?1 AND status = 'published'",
    )?;
    let rows = st.query_map(params![ext], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut best: Option<(semver::Version, String)> = None;
    for r in rows {
        let (v, h) = r?;
        if let Ok(v) = semver::Version::parse(&v) {
            if best.as_ref().map_or(true, |(b, _)| v > *b) {
                best = Some((v, h));
            }
        }
    }
    Ok(best)
}

fn summary(db: &Connection, id: &str, name: &str, desc: &str) -> rusqlite::Result<Option<Value>> {
    let Some((ver, digest)) = latest_published(db, id)? else {
        return Ok(None);
    };
    let (avg, cnt): (Option<f64>, i64) = db.query_row(
        "SELECT AVG(stars), COUNT(*) FROM ratings WHERE ext_id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(Some(
        json!({ "id": id, "name": name, "description": desc, "latest_version": ver.to_string(), "sha256": digest,
                    "rating_average": avg, "rating_count": cnt }),
    ))
}

async fn search(State(s): State<AppState>, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);
    let pat = format!(
        "%{}%",
        q.q.unwrap_or_default()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let db = s.0.db.lock().unwrap();
    let mut st = db.prepare(
        "SELECT id, name, description FROM extensions e WHERE (id LIKE ?1 ESCAPE '\\' OR name LIKE ?1 ESCAPE '\\' OR description LIKE ?1 ESCAPE '\\') \
         AND EXISTS(SELECT 1 FROM versions v WHERE v.ext_id = e.id AND v.status='published') ORDER BY id LIMIT ?2 OFFSET ?3",
    )?;
    let rows = st
        .query_map(params![pat, limit, offset], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (id, n, d) in rows {
        if let Some(v) = summary(&db, &id, &n, &d)? {
            out.push(v);
        }
    }
    Ok(Json(
        json!({ "results": out, "limit": limit, "offset": offset }),
    ))
}

async fn details(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let db = s.0.db.lock().unwrap();
    let ext: Option<(String, String, String)> = db
        .query_row("SELECT e.name, e.description, u.username FROM extensions e JOIN users u ON u.id = e.publisher_id WHERE e.id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    let Some((name, desc, publisher)) = ext else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "extension not found"));
    };
    let Some(mut v) = summary(&db, &id, &name, &desc)? else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "extension not found"));
    };
    let mut st = db.prepare("SELECT version, sha256, size, signature, published_at FROM versions WHERE ext_id = ?1 AND status='published' ORDER BY published_at, id")?;
    let versions = st
        .query_map(params![id], |r| {
            Ok(json!({ "version": r.get::<_, String>(0)?, "sha256": r.get::<_, String>(1)?, "size": r.get::<_, i64>(2)?,
                       "signature": r.get::<_, Option<String>>(3)?, "published_at": r.get::<_, Option<i64>>(4)? }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    v["publisher"] = json!(publisher);
    v["versions"] = json!(versions);
    Ok(Json(v))
}

async fn download(
    State(s): State<AppState>,
    Path((id, version)): Path<(String, String)>,
) -> ApiResult<Response> {
    let key = s.signer()?.verifying_key();
    let db = s.0.db.lock().unwrap();
    let row: Option<(Vec<u8>, String, Option<String>)> = db
        .query_row("SELECT package, sha256, signature FROM versions WHERE ext_id=?1 AND version=?2 AND status='published'", params![id, version], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    drop(db);
    let Some((pkg, digest, sig)) = row else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "version not found"));
    };
    let raw = Sha256::digest(&pkg);
    let sig_hex = sig.unwrap_or_default();
    let verified = hex::encode(raw) == digest
        && hex::decode(&sig_hex)
            .ok()
            .and_then(|b| Signature::from_slice(&b).ok())
            .is_some_and(|sg| key.verify(&raw, &sg).is_ok());
    if !verified {
        return Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stored package failed hash/signature verification",
        ));
    }
    let disp = HeaderValue::from_str(&format!(
        "attachment; filename=\"{id}-{version}.nortex.zip\""
    ))
    .map_err(ApiError::internal)?;
    let dh = HeaderValue::from_str(&digest).map_err(ApiError::internal)?;
    let sh = HeaderValue::from_str(&sig_hex).map_err(ApiError::internal)?;
    let mut resp = pkg.into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    h.insert(header::CONTENT_DISPOSITION, disp);
    h.insert("x-nortex-sha256", dh);
    h.insert("x-nortex-signature", sh);
    Ok(resp)
}

fn require_published(db: &Connection, id: &str) -> ApiResult<()> {
    let ok: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM versions WHERE ext_id=?1 AND status='published')",
        params![id],
        |r| r.get(0),
    )?;
    if ok {
        Ok(())
    } else {
        Err(ApiError::new(StatusCode::NOT_FOUND, "extension not found"))
    }
}

#[derive(Deserialize)]
struct RatingBody {
    stars: i64,
}

async fn rate(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(b): Json<RatingBody>,
) -> ApiResult<Json<Value>> {
    let u = s.authenticate(&headers)?;
    if !(1..=5).contains(&b.stars) {
        return Err(ApiError::bad("stars must be 1-5"));
    }
    let db = s.0.db.lock().unwrap();
    require_published(&db, &id)?;
    db.execute(
        "INSERT INTO ratings(user_id, ext_id, stars, updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(user_id, ext_id) DO UPDATE SET stars=excluded.stars, updated_at=excluded.updated_at",
        params![u.id, id, b.stars, now()],
    )?;
    Ok(Json(json!({ "id": id, "stars": b.stars })))
}

async fn favorite_add(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let u = s.authenticate(&headers)?;
    let db = s.0.db.lock().unwrap();
    require_published(&db, &id)?;
    db.execute(
        "INSERT OR IGNORE INTO favorites(user_id, ext_id) VALUES(?1, ?2)",
        params![u.id, id],
    )?;
    Ok(StatusCode::NO_CONTENT)
}

async fn favorite_remove(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let u = s.authenticate(&headers)?;
    s.0.db.lock().unwrap().execute(
        "DELETE FROM favorites WHERE user_id=?1 AND ext_id=?2",
        params![u.id, id],
    )?;
    Ok(StatusCode::NO_CONTENT)
}

async fn favorites_list(State(s): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let u = s.authenticate(&headers)?;
    let db = s.0.db.lock().unwrap();
    let mut st = db.prepare("SELECT e.id, e.name, e.description FROM favorites f JOIN extensions e ON e.id = f.ext_id WHERE f.user_id=?1 ORDER BY e.id")?;
    let rows = st
        .query_map(params![u.id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (id, n, d) in rows {
        if let Some(v) = summary(&db, &id, &n, &d)? {
            out.push(v);
        }
    }
    Ok(Json(json!({ "favorites": out })))
}

#[derive(Deserialize)]
struct Installed {
    id: String,
    version: String,
}

#[derive(Deserialize)]
struct UpdateReq {
    installed: Vec<Installed>,
}

async fn updates_check(
    State(s): State<AppState>,
    Json(req): Json<UpdateReq>,
) -> ApiResult<Json<Value>> {
    if req.installed.len() > MAX_UPDATE_ITEMS {
        return Err(ApiError::bad(format!(
            "at most {MAX_UPDATE_ITEMS} installed extensions per request"
        )));
    }
    let db = s.0.db.lock().unwrap();
    let mut out = Vec::new();
    for i in req.installed {
        let cur = semver::Version::parse(&i.version)
            .map_err(|_| ApiError::bad(format!("invalid version for {}", i.id)))?;
        if let Some((latest, digest)) = latest_published(&db, &i.id)? {
            if latest > cur {
                out.push(json!({ "id": i.id, "installed_version": i.version, "latest_version": latest.to_string(), "sha256": digest,
                                 "download": format!("/api/v1/extensions/{}/versions/{}/download", i.id, latest) }));
            }
        }
    }
    Ok(Json(json!({ "updates": out })))
}
