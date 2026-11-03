# NORTEX Code – Plugin System

Marketplace extensions are untrusted by default. The IDE requires both local approval and
activation before offering their executable entry point.

1. A discovered plugin is registered as `Pending`.
2. Pending plugins cannot be enabled.
3. The user may approve or reject a plugin in **Extensions → Installed**.
4. Rejected plugins remain disabled. Re-registering an existing ID updates its name without
   resetting its approval or activation state.
5. Approval, activation, and sidebar visibility are stored independently. Hiding a sidebar entry
   does not activate or deactivate the plugin. Running an extension still requires a separate
   confirmation for that run.
6. Registry state is stored in `%APPDATA%\NORTEX Code\extensions.json` on Windows. A malformed
   registry is reported in the IDE status rather than silently overwritten.

The registry is serialized by `nortex_ide::plugins::PluginRegistry`. Approval is not a security
review and must not be presented as one.

The Extensions screen exposes Marketplace, Installed, Publishing, Updates, and Favorites. The IDE
can connect to the self-hosted API for search, accounts, ratings, favorites, uploads, update checks,
and package installation. Marketplace tokens remain in memory. Publisher uploads require the
account to have a publisher or admin role.

Installation requires an independently obtained and configured Ed25519 public key. The installer
verifies the package digest and signature, validates manifest and archive paths, checks the NORTEX
version range and dependency graph, and stages package replacements for rollback on failure. New
installs and updates require local approval again.

The manifest may declare `nortex`, `dependencies`, and `permissions`. When an extension's dependency
is missing or does not satisfy its semver range, installation resolves the newest published
compatible version recursively, verifies every archive signature, and installs the dependency graph
before the requested extension. The batch is rolled back on installation failure. Newly installed
or updated dependencies remain pending approval and are never enabled automatically. Resolution is
limited to 128 packages and 32 dependency levels; there is no offline dependency cache. Missing,
incompatible, cyclic, or conflicting dependency requirements fail installation. Permission names
are limited to `network.client`, `network.private`, `filesystem.package.read`, and
`filesystem.package.write`. A Marketplace package can declare a `.nx` source entry or `.exe` entry.
NORTEX sources are compiled locally with the installed NORTEX compiler after per-run confirmation.
Both run only while approved and enabled, and only after the user reviews and confirms requested
permissions for that run. On Windows, the IDE launches them in an AppContainer and a
resource-limited Job Object; sandbox creation errors fail closed. File access is scoped to the
extension's own install directory, and requested network capabilities are granted only when
declared.

## Plugin host and IDE API

An extension whose `nortex-extension.json` declares `ide_api` runs as a **hosted plugin**; without
it, the entry point is a **batch process** that is started, waited for and reported (stdout, stderr,
exit code). Both kinds start only after the per-run confirmation and only inside the sandbox.

```json
{
  "id": "acme.status",
  "entry": "main.nx",
  "permissions": [],
  "ide_api": ">=1.0.0, <2.0.0",
  "ide_permissions": ["ide.log", "ide.status.write", "ide.document.read"]
}
```

`ide_api` is a semver requirement; it states both the minimum and the maximum supported API
version. The IDE currently provides API **1.2.0** and protocol **1**. `ide_permissions` are separate
from the sandbox `permissions`: they control what the plugin may ask the IDE, not what the process
may do on the machine. Unknown or duplicate names and more than 16 entries are rejected, both at
installation and at start. `ide_permissions` without `ide_api` is rejected.

### Transport and messages

The host talks to the plugin over its stdin and stdout, one JSON object per line (UTF-8, `\n`).
stderr is discarded. A line may not exceed 64 KiB; a longer line, invalid JSON, an unknown field or
a wrong protocol number ends the session. Frames are
`{"protocol":1,"id":N,"type":"request"|"response","method":…,"params":…,"result":…,"error":{"code","message"}}`.

Handshake: the host sends request `initialize` (`protocol`, `ide_api`, `plugin_id`, the callable
`methods`). The plugin answers with `result.ide_api` (its own requirement, checked against the IDE
version) plus optional `name` and `version`. No answer within 5 s, or an incompatible requirement,
fails the session. Requests before the handshake completes get `not_initialized`.

The `initialize` result may also declare up to 16 user-invokable actions:
`"actions":[{"id":"format","title":"Format document"}]`. IDs are unique lowercase
`[a-z0-9._-]` strings of at most 48 characters; titles are 1–60 characters and cannot contain
control characters. Invalid or duplicate declarations fail the handshake. Active actions appear
in the **Extensions** menu. Selecting one sends the host-to-plugin request
`action.invoke` with its declared ID. Only one action may run per plugin at a time; it must
respond within 10 seconds or the host terminates the plugin. The response or error is shown in the
IDE status bar. Pending actions fail when the plugin stops, and actions disappear from the menu
when it is no longer active. Restart still requires the user to stop and start the plugin again.

| Method | Permission | Result |
|---|---|---|
| `log` (`level` info/warn/error, `message`) | `ide.log` | `{}`; kept in a ring of 200 entries per plugin |
| `status.set` (`text`) | `ide.status.write` | `{}`; shown in the Installed list (120 characters) |
| `document.active` | `ide.document.read` | file name (no path), language, text (cut at 24 KiB), `truncated`; `null` without a document |
| `project.files` | `ide.project.read` | project-relative paths, at most 500, `truncated` |
| `diagnostics.active` | `ide.diagnostics.read` | diagnostics of the active document, at most 200 |
| `document.replace` (`text`, ≤ 32 KiB) | `ide.document.write` | `{}`; replaces the active document text (undoable); since 1.1 |
| `file.read` (`path`) | `ide.file.read` | `{text}`; project-relative path, UTF-8, ≤ 1 MiB; since 1.1 |
| `file.write` (`path`, `text` ≤ 32 KiB) | `ide.file.write` | `{}`; project-relative, directory must exist, no symlinks; since 1.1 |
| `notification.show` (`text`) | `ide.notification` | `{}`; shown in the status bar; since 1.1 |
| `search.text` (`query`) | `ide.search` | `{matches:[{file,line,text}]}`, at most 100; since 1.1 |
| `git.status` | `ide.git.read` | `{branch, changes:[{status,path}]}` or `null`; since 1.1 |
| `theme.current` | `ide.theme.read` | `{name:"dark"\|"light"}`; since 1.1 |
| `archive.create` (`output`, `files`, `include_active_document`) | `ide.archive.write` | `{output,entries,archive_bytes}`; writes a real ZIP (Deflate) `.nxpack`; since 1.2 |
| `document.export_pdf` (`output`) | `ide.document.export` | `{output,pages,bytes}`; writes the active document as a Unicode PDF (formatted when it starts with `%%nxpdf 1`, see below); since 1.2 |

Errors: `unknown_method`, `permission_denied`, `invalid_params`, `not_initialized`, `unavailable`,
`rate_limited`. More than 100 requests per second end the session. Paths that are absolute,
contain `..` or leave the project (also through symlinks) are rejected with `unavailable`.

### NORTEX SDK

Plugins can be written in NORTEX itself: `extensions/sdk/sdk.nx` implements the framing,
the handshake and wrappers for the methods above using only `Console.readLine`,
`Console.writeLine` and `Text.*`. A package includes a copy of `sdk.nx` and imports it with
`using sdk;`. `extensions/bundled/formatter` is the reference. Strings passed to user functions
are moved, so reused arguments are passed as `value + ""`.

### Lifecycle

`Starting → Active → Stopping → Stopped`, or `Failed(reason)` with: incompatible, handshake
timeout, protocol violation, message too large, rate limited, process exited, I/O error. Disabling
or rejecting the plugin, the **Beenden** button, or closing the IDE stops it: the host sends
`shutdown`, waits 2 s and then kills the process; closing the Job Object also kills it. A failed
plugin is never restarted automatically; the user starts it again manually. A compiled `.nx`
executable is deleted once the session has ended.

The reader and writer threads own the pipes, the UI thread only drains queued frames (at most 32
per frame) and never waits for a plugin. A plugin that stops reading its input is killed when the
64-frame outgoing queue is full.

Code: `ide/src/plugins/protocol.rs` (frames, versions, manifest), `ide/src/plugins/host.rs`
(sessions, API dispatch), `ide/src/sandbox.rs` (`spawn_sandboxed`, interactive pipes),
`ide/src/shell/extensions.rs` (integration). Tests: `ide/tests/plugin_ipc.rs` runs the host against
a real child process (`ide/examples/plugin_stub.rs`): start, API calls, denied permission, stop,
kill after ignored shutdown, incompatible API (manifest and handshake), invalid and oversized
messages, request flood, handshake timeout, crash, unlaunchable process, double start. The ignored
test `plugin_runs_hosted_inside_the_appcontainer_sandbox` runs the same exchange inside the
AppContainer. Plugin processes are started through `std::process::Command` only in these tests; the
IDE itself has no launcher without a sandbox.

### Archive and PDF export (API 1.2)

`archive.create` packs project files (and optionally the current text of the active document,
stored as its file name) into a `.nxpack` ZIP with Deflate; `document.export_pdf` writes the whole
active document to a `.pdf` file. Both need their own permission (`ide.archive.write`,
`ide.document.export`) and fail closed: output and input paths must be project-relative with the
right extension, symlinks/junctions are refused, and the file is written atomically. Limits: at most
200 entries, 4 MiB per file, 16 MiB uncompressed and 16 MiB archive; PDF at most 1 MiB of text and
400 pages (A4). Exceeding a limit is an error, never a truncation.

#### PDF content and NXPDF markup

The PDF embeds a TrueType font (Ubuntu Light for markup, Hack for plain text; both ship with the IDE)
as a Unicode CID font with a `ToUnicode` map, so text stays selectable and Latin, Greek and Cyrillic
render correctly. Characters the font lacks (e.g. CJK, emoji) show the font's "missing glyph" box.
Output is deterministic. Images and free positioning are not supported.

* **Plain-text fallback**: a document whose first line is not `%%nxpdf 1` is written verbatim in
  9 pt monospace (tabs at 4 columns, hard wrap at the margin) with a centred `n / N` footer.
  Nothing is interpreted, so source code exports unchanged. `%%nxpdf <other version>` is rejected.
* **NXPDF markup**: the first line is exactly `%%nxpdf 1` (optional UTF-8 BOM). Line by line:

| Syntax | Meaning |
|---|---|
| `# `, `## `, `### ` + text | Heading level 1-3 (20/16/13 pt, bold) |
| text lines, blank line between | Paragraph (11 pt); consecutive lines are joined with a space and word-wrapped |
| `**bold**`, `*italic*` | Bold (stroked) / italic (slanted); may be combined; closed at the end of a block |
| `{#rrggbb}text{/}` or `{#rgb}` | Text colour; nestable; invalid specs stay literal |
| `@pagebreak` | Explicit page break (ignored on an empty page; trailing breaks add no page) |
| `@header a \| b \| c`, `@footer ...` | Header/footer on every page; last occurrence wins; max. 200 characters |
| `%% comment` | Ignored line |
| `\*`, `\{`, `\}`, `\|`, `\#`, `\@`, `\\` | Literal character; `\n` inside a paragraph is a hard line break |

Header/footer text takes the same inline markup plus `{page}` and `{pages}`. Zones are separated by
`|`: one part is centred, two parts are left/right, three parts are left/centre/right (more is an
error). Unknown `@...` lines are ordinary text. Example:

```
%%nxpdf 1
@header Report | **Draft**
@footer Page {page} of {pages}
# Title
Some **bold**, *italic* and {#c00000}red{/} text.
@pagebreak
## Next page
```

The SDK offers `sdk_init_actions`, `sdk_wait_action`, `sdk_action_done`/`sdk_action_failed`,
`sdk_create_archive` and `sdk_export_pdf`. The extensions `compression` (`pack-active`),
`archive-tools` (`pack-project`) and `pdf-tools` (`export-pdf`) use them as menu actions.

### Limits

- Hosted plugins and batch runs exist only on Windows. Linux and macOS refuse to start any
  plugin (`Unsupported`); there is no unsandboxed fallback. The non-Windows branch of
  `ide/src/sandbox.rs` could not be compiled here (no Linux toolchain for the C dependencies), so
  it is unverified.
- The host runs in the IDE process. It is not a sandbox; isolation comes from the plugin's
  process. A plugin cannot be loaded as a library.
- The NORTEX compiler that turns a `.nx` entry into an executable runs in a Job Object (process
  count 16, 2 GiB, 120 s CPU, below-normal priority, UI restrictions) with a Low-Integrity
  token. The process is created suspended, assigned to the job and only then resumed; if the
  assignment fails it is killed without having run. Only stdin (NUL), stdout and stderr are
  inherited. It works on a hash-checked snapshot copy; writing is possible only inside the
  labelled workspace. **Reading and network access are not restricted**; it is not an
  AppContainer. Whether the vcvars cache under `LOCALAPPDATA` is usable at Low integrity was not
  measured.
- Smoke tests on Windows need an AppContainer-capable host, `nortexc` next to the test binary and
  permission to change ACLs in the temp directory:
  `cargo test -p nortex-ide --lib confine -- --include-ignored --test-threads=1`, same for
  `sandbox`, plus `--test plugin_ipc`.
- Dependencies must already be installed; plugin dependencies are checked at installation only.
- No automatic restart, no more than one batch run at a time, IDE API 1.2.0.

See
[Marketplace architecture and backend requirements](marketplace.md) for the service boundary and
deployment prerequisites.
