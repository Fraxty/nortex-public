# NORTEX Marketplace backend

Self-hostable Rust (axum + SQLite) backend in the NORTEX workspace. Contract: [`openapi.yaml`](openapi.yaml).

## Run

```powershell
# Admin bootstrap (password only via environment; nothing is hard-coded)
$env:NORTEX_MARKET_ADMIN_PASSWORD = "<choose a strong password>"
cargo run --release -p nortex-marketplace -- create-admin <username>
# Signing key: 32-byte Ed25519 seed as 64 hex chars, generated and stored by you (never commit it)
$env:NORTEX_MARKET_SIGNING_KEY_HEX = "<64 hex chars>"
cargo run --release -p nortex-marketplace
```

| Variable | Default | Purpose |
|---|---|---|
| `NORTEX_MARKET_BIND` | `127.0.0.1:8787` | Listen address (loopback by default) |
| `NORTEX_MARKET_DB` | `nortex-marketplace.sqlite3` | SQLite file |
| `NORTEX_MARKET_MAX_UPLOAD_BYTES` | `10485760` | Request body limit (413 above) |
| `NORTEX_MARKET_TOKEN_TTL_SECS` | `86400` | Bearer token lifetime |
| `NORTEX_MARKET_SIGNING_KEY_HEX` | unset | Ed25519 seed; without it approve/download/signing-key return 503 |

## Public hosting

The server speaks plain HTTP only. To expose it, keep it on loopback and put a reverse proxy (Caddy, nginx, ...) in front that terminates HTTPS and forwards to `127.0.0.1:8787`. Set the proxy's body limit to match `NORTEX_MARKET_MAX_UPLOAD_BYTES`. Never send credentials over plain HTTP.

## Model

- Roles: `user` (self-registered), `publisher`, `admin`. Admins grant roles (`PUT /api/v1/admin/users/{u}/role`) and approve uploads. Authorization is enforced server-side.
- Passwords: Argon2id. Tokens: 256-bit random, stored only as SHA-256, expire server-side, revocable via logout.
- Package: zip with `nortex-extension.json` (`id`, `name`, `version` canonical semver, optional `description`, `entry`). Archive entry paths are validated (no absolute/`..`/backslash/drive/symlink/duplicates, entry and size caps). Archives are never extracted or executed.
- `.nxpack` utility API (`nortex_marketplace::nxpack`): `pack_nxpack` and `unpack_nxpack` read/write standard Deflate ZIP files, returning file paths and bytes in memory. It is a generic project archive format, not a signed marketplace extension package; no manifest is required. Both directions reject unsafe paths, duplicates and symbolic links, with limits of 5,000 entries, 256 MiB uncompressed and 256 MiB compressed. The unpacker does not write to disk; callers must preserve these path checks if they later extract files.
- Uploads are `pending`; admin approval signs the SHA-256 digest with Ed25519 and publishes. Downloads re-verify hash and signature, returning `X-NORTEX-SHA256` / `X-NORTEX-Signature`. Clients should verify with `GET /api/v1/signing-key` pinned out-of-band.
- `POST /api/v1/extensions/{id}/run` returns 501: no runtime hosting exists.

## Test

`cargo test -p nortex-marketplace` (integration tests in `marketplace/tests/api.rs`).

## Known gaps

- No rate limiting / login throttling (use the reverse proxy).
- No publisher-supplied signatures; only server signing. No key rotation or revocation/unpublish endpoint.
- Marketplace uploads are validated but never decompressed or extracted by the server; clients must still use the bounded `.nxpack` unpacker or extract defensively.
- Packages are stored as SQLite BLOBs (fine for small hosts); no password change/reset, no email verification.
- Single SQLite connection behind a mutex.
