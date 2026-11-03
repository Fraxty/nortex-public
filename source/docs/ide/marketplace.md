# NORTEX Code – Marketplace Architecture and Backend Requirements

## Current implementation

The repository includes a self-hostable Axum/SQLite service in `marketplace/`. Its versioned API
supports account registration/login/logout, extension search/details, publisher uploads, admin
review and signing, downloads, ratings, favorites, and update checks. See
[`marketplace/README.md`](../../marketplace/README.md) and its [`openapi.yaml`](../../marketplace/openapi.yaml)
for local setup and the API contract.

The desktop Extensions view has an API client for search, account registration/login/logout,
ratings, favorites, publisher uploads, admin review/role changes, and update checks. Sessions are
held in memory only. The client rejects plain HTTP except on loopback and does not follow redirects.
Package installation requires an independently pinned Ed25519 public key, verifies the downloaded
digest and signature, validates the manifest and archive paths, checks NORTEX compatibility and
dependency constraints, extracts into a staging directory, and swaps the install directory only
after validation. Newly installed or updated packages remain pending approval.

## Required service boundary

A production marketplace needs a versioned HTTPS API and an independently deployed server. At a
minimum, its API should define:

| Area | Required server operation |
|---|---|
| Discovery | Implemented: text search with offset pagination in the API and IDE; categories, tags, sorting, and compatibility filters remain |
| Details | Implemented in API: description, publisher, ratings, and published versions; localized metadata, changelogs, and media remain |
| Downloads | Implemented: published package, SHA-256 digest, pinned-key signature verification, strict archive checks, and staged install |
| Updates | Implemented: installed manifest scan, latest-version comparison, and client-side compatibility/dependency checks; server-side compatibility filtering and retention policy remain |
| Accounts | Implemented: username/password registration/login/logout and user/publisher/admin roles; tokens are memory-only; publisher identity verification and account recovery remain |
| Publishing | Implemented: bounded upload, manifest/archive checks (including permission and version-range validation), pending review with manifest metadata, admin approve/reject; validation reports and audit history remain |
| Ratings | Implemented: authenticated create/update and server-calculated aggregate; reporting/moderation remain |
| Favorites | Implemented: authenticated add/remove/list |
| Verification | Admin review exists; audited decisions and NORTEX ownership verification remain |

The service binds to loopback and speaks HTTP. Public deployment requires a TLS reverse proxy.
The client uses authenticated HTTPS outside loopback, validates package signatures against a
manually configured key, and treats remote metadata and archives as untrusted. The current service
has no rate limiting, login throttling, password recovery, or review audit history.

## Client implementation constraints

Do not populate the production Marketplace with built-in demo records or locally invented
statistics. A missing endpoint or account service must remain an explicit unavailable/error state.
Do not install an archive merely because it downloaded successfully: manifest validation,
path traversal protection, signature verification, compatibility checks, installed dependency
resolution, and rollback handling must complete before changing the local installation. Missing dependencies are resolved recursively from published Marketplace versions. All package
signatures are verified before installation, the dependency graph is bounded and cycle-checked, and
the batch installer rolls back on installation failure. Dependencies are not activated or approved
automatically.

Configure the IDE endpoint with `NORTEX_MARKETPLACE_URL` (defaults to loopback) and the pinned
public-key hex with `NORTEX_MARKET_SIGNING_PUBLIC_KEY_HEX`, or enter the key in the Marketplace
view. Obtain the public key through a trusted channel independent of the marketplace connection;
the API's signing-key response is not itself a trust anchor.

On Windows, approved and activated `.exe` extension entries and locally compiled `.nx` entries run
in an AppContainer and Job Object after per-run permission confirmation. Without `ide_api` in the
manifest they are one-shot batch processes; with `ide_api` they are hosted plugins that talk to the
IDE over a versioned JSON-lines IPC (see [plugins.md](plugins.md)). The manifest validation at
installation also checks `ide_api` and `ide_permissions`. Other operating systems remain
unsupported and refuse to start extensions.
