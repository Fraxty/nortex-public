// Der Online-Client (Auth, Suche, Bewertungen) hat vorerst keine Oberfläche; der
// Marketplace-Tab zeigt den lokalen Katalog aus extensions/available.
#![allow(dead_code)]

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::blocking::{Client, Response};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use zip::ZipArchive;

const MAX_DOWNLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 5000;
const MAX_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MANIFEST_NAME: &str = "nortex-extension.json";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MarketplaceExtension {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub latest_version: String,
    #[serde(default)]
    pub rating_average: f64,
    #[serde(default)]
    pub rating_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MarketplaceUpdate {
    pub id: String,
    pub installed_version: String,
    pub latest_version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MarketplaceSubmission {
    pub submission_id: i64,
    pub id: String,
    pub version: String,
    pub size: i64,
    pub sha256: String,
    pub publisher: String,
    pub manifest: String,
}

#[derive(Debug, Serialize)]
struct InstalledVersion {
    id: String,
    version: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<MarketplaceExtension>,
}

#[derive(Deserialize)]
struct VersionsResponse {
    versions: Vec<MarketplaceVersion>,
}

#[derive(Deserialize)]
struct MarketplaceVersion {
    version: String,
}

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
    role: String,
}

struct DownloadedPackage {
    bytes: Vec<u8>,
    digest: String,
    signature: String,
}

#[derive(Clone, Deserialize)]
struct PackageManifest {
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
    dependencies: HashMap<String, String>,
    #[serde(default)]
    permissions: Vec<String>,
}

pub(crate) struct MarketplaceClient {
    base_url: String,
    http: Client,
}

impl MarketplaceClient {
    pub(crate) fn new(base_url: &str) -> Result<Self, String> {
        let url = reqwest::Url::parse(base_url.trim()).map_err(|error| error.to_string())?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !matches!(url.path(), "" | "/")
        {
            return Err("Marketplace URL must contain only an origin".into());
        }
        let host = url
            .host_str()
            .ok_or_else(|| "Marketplace URL has no host".to_string())?;
        let local = host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if url.scheme() != "https" && !(url.scheme() == "http" && local) {
            return Err(
                "Marketplace authentication requires HTTPS; HTTP is allowed only on loopback"
                    .into(),
            );
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            base_url: url.as_str().trim_end_matches('/').to_owned(),
            http,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn check(response: Response) -> Result<Response, String> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let mut body = String::new();
        let _ = response.take(4096).read_to_string(&mut body);
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value.get("error")?.as_str().map(str::to_owned))
            .unwrap_or_else(|| body.chars().take(300).collect());
        Err(format!("Marketplace request failed ({status}): {detail}"))
    }

    fn parse_json<T: DeserializeOwned>(response: Response, max_bytes: usize) -> Result<T, String> {
        let mut bytes = Vec::new();
        response
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > max_bytes {
            return Err("Marketplace response exceeds the client size limit".into());
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("Invalid marketplace response: {error}"))
    }

    pub(crate) fn search(&self, query: &str) -> Result<Vec<MarketplaceExtension>, String> {
        let response = self
            .http
            .get(self.endpoint("/api/v1/extensions"))
            .query(&[("q", query)])
            .send()
            .map_err(|error| error.to_string())?;
        Self::parse_json::<SearchResponse>(Self::check(response)?, 2 * 1024 * 1024)
            .map(|result| result.results)
    }

    fn versions(&self, id: &str) -> Result<Vec<semver::Version>, String> {
        validate_extension_id(id)?;
        let response = self
            .http
            .get(self.endpoint(&format!("/api/v1/extensions/{id}")))
            .send()
            .map_err(|error| error.to_string())?;
        let versions = Self::parse_json::<VersionsResponse>(Self::check(response)?, 512 * 1024)?
            .versions
            .into_iter()
            .map(|entry| {
                semver::Version::parse(&entry.version)
                    .map_err(|error| format!("Marketplace returned an invalid version: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(versions)
    }

    pub(crate) fn login(&self, username: &str, password: &str) -> Result<(String, String), String> {
        let response = self
            .http
            .post(self.endpoint("/api/v1/auth/login"))
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .map_err(|error| error.to_string())?;
        let auth = Self::parse_json::<AuthResponse>(Self::check(response)?, 16 * 1024)?;
        Ok((auth.token, auth.role))
    }

    pub(crate) fn register(&self, username: &str, password: &str) -> Result<(), String> {
        let response = self
            .http
            .post(self.endpoint("/api/v1/auth/register"))
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .map_err(|error| error.to_string())?;
        Self::check(response).map(|_| ())
    }

    pub(crate) fn favorites(&self, token: &str) -> Result<Vec<MarketplaceExtension>, String> {
        let response = self
            .http
            .get(self.endpoint("/api/v1/me/favorites"))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .map_err(|error| error.to_string())?;
        #[derive(Deserialize)]
        struct FavoritesResponse {
            favorites: Vec<MarketplaceExtension>,
        }
        Self::parse_json::<FavoritesResponse>(Self::check(response)?, 2 * 1024 * 1024)
            .map(|result| result.favorites)
    }

    pub(crate) fn logout(&self, token: &str) -> Result<(), String> {
        let response = self
            .http
            .post(self.endpoint("/api/v1/auth/logout"))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .map_err(|error| error.to_string())?;
        Self::check(response).map(|_| ())
    }

    pub(crate) fn submissions(&self, token: &str) -> Result<Vec<MarketplaceSubmission>, String> {
        #[derive(Deserialize)]
        struct SubmissionsResponse {
            submissions: Vec<MarketplaceSubmission>,
        }
        let response = self
            .http
            .get(self.endpoint("/api/v1/admin/submissions"))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .map_err(|error| error.to_string())?;
        Self::parse_json::<SubmissionsResponse>(Self::check(response)?, 512 * 1024)
            .map(|result| result.submissions)
    }

    pub(crate) fn review_submission(
        &self,
        token: &str,
        submission_id: i64,
        approve: bool,
    ) -> Result<(), String> {
        let action = if approve { "approve" } else { "reject" };
        let response = self
            .http
            .post(self.endpoint(&format!(
                "/api/v1/admin/submissions/{submission_id}/{action}"
            )))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .map_err(|error| error.to_string())?;
        Self::check(response).map(|_| ())
    }

    pub(crate) fn set_role(&self, token: &str, username: &str, role: &str) -> Result<(), String> {
        let response = self
            .http
            .put(self.endpoint(&format!("/api/v1/admin/users/{username}/role")))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .json(&serde_json::json!({ "role": role }))
            .send()
            .map_err(|error| error.to_string())?;
        Self::check(response).map(|_| ())
    }

    pub(crate) fn favorite(&self, token: &str, id: &str) -> Result<(), String> {
        let response = self
            .http
            .put(self.endpoint(&format!("/api/v1/extensions/{id}/favorite")))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .map_err(|error| error.to_string())?;
        Self::check(response).map(|_| ())
    }

    pub(crate) fn rate(&self, token: &str, id: &str, stars: u8) -> Result<(), String> {
        let response = self
            .http
            .put(self.endpoint(&format!("/api/v1/extensions/{id}/rating")))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({ "stars": stars }))
            .send()
            .map_err(|error| error.to_string())?;
        Self::check(response).map(|_| ())
    }

    pub(crate) fn upload(&self, token: &str, package: Vec<u8>) -> Result<i64, String> {
        let response = self
            .http
            .post(self.endpoint("/api/v1/extensions"))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/zip")
            .body(package)
            .send()
            .map_err(|error| error.to_string())?;
        #[derive(Deserialize)]
        struct UploadResponse {
            submission_id: i64,
        }
        Self::parse_json::<UploadResponse>(Self::check(response)?, 16 * 1024)
            .map(|result| result.submission_id)
    }

    pub(crate) fn check_updates(
        &self,
        installed: Vec<(String, String)>,
    ) -> Result<Vec<MarketplaceUpdate>, String> {
        if installed.len() > 500 {
            return Err("At most 500 installed extensions can be checked at once".into());
        }
        #[derive(Deserialize)]
        struct UpdatesResponse {
            updates: Vec<MarketplaceUpdate>,
        }
        let installed = installed
            .into_iter()
            .map(|(id, version)| InstalledVersion { id, version })
            .collect::<Vec<_>>();
        let response = self
            .http
            .post(self.endpoint("/api/v1/updates/check"))
            .json(&serde_json::json!({ "installed": installed }))
            .send()
            .map_err(|error| error.to_string())?;
        Self::parse_json::<UpdatesResponse>(Self::check(response)?, 256 * 1024)
            .map(|result| result.updates)
    }

    fn download(&self, id: &str, version: &str) -> Result<DownloadedPackage, String> {
        validate_extension_version(id, version)?;
        let response = self
            .http
            .get(self.endpoint(&format!(
                "/api/v1/extensions/{id}/versions/{version}/download"
            )))
            .send()
            .map_err(|error| error.to_string())?;
        let response = Self::check(response)?;
        let digest = response
            .headers()
            .get("x-nortex-sha256")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "Marketplace response has no package digest".to_string())?
            .to_owned();
        let signature = response
            .headers()
            .get("x-nortex-signature")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "Marketplace response has no package signature".to_string())?
            .to_owned();
        if response
            .content_length()
            .is_some_and(|length| length > MAX_DOWNLOAD_BYTES as u64)
        {
            return Err("Package exceeds the 64 MiB download limit".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_DOWNLOAD_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > MAX_DOWNLOAD_BYTES {
            return Err("Package exceeds the 64 MiB download limit".into());
        }
        Ok(DownloadedPackage {
            bytes,
            digest,
            signature,
        })
    }
}

pub(crate) fn installed_versions(install_root: &Path) -> Result<Vec<(String, String)>, String> {
    let entries = match fs::read_dir(install_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut installed = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if id.starts_with('.') {
            continue;
        }
        validate_extension_id(&id)?;
        let manifest_path = entry.path().join(MANIFEST_NAME);
        let mut bytes = Vec::new();
        fs::File::open(manifest_path)
            .map_err(|error| error.to_string())?
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(format!(
                "Installed extension {id} has an oversized manifest"
            ));
        }
        let manifest: PackageManifest = serde_json::from_slice(&bytes).map_err(|error| {
            format!("Installed extension {id} has an invalid manifest: {error}")
        })?;
        validate_extension_version(&manifest.id, &manifest.version)?;
        if manifest.id != id {
            return Err(format!(
                "Installed extension folder {id} does not match its manifest ID"
            ));
        }
        installed.push((id, manifest.version));
    }
    Ok(installed)
}

pub(crate) fn download_and_install(
    base_url: &str,
    id: &str,
    version: &str,
    trusted_key_hex: &str,
    install_root: &Path,
) -> Result<Vec<(String, String)>, String> {
    download_and_install_with_dependencies(base_url, id, version, trusted_key_hex, install_root)
}

const LOCK_STALE_AFTER: Duration = Duration::from_secs(15 * 60);

/// Serializes installations into one extension directory. The lock file is
/// created exclusively; one left behind by a crash is taken over after
/// `LOCK_STALE_AFTER`.
struct InstallLock(PathBuf);

impl InstallLock {
    fn acquire(install_root: &Path) -> Result<Self, String> {
        fs::create_dir_all(install_root).map_err(|error| error.to_string())?;
        let path = install_root.join(".nortex-install.lock");
        for _ in 0..2 {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(Self(path));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| modified.elapsed().ok())
                        .is_some_and(|age| age > LOCK_STALE_AFTER);
                    if !stale || fs::remove_file(&path).is_err() {
                        return Err("Another extension installation is in progress".into());
                    }
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("Another extension installation is in progress".into())
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn download_and_install_with_dependencies(
    base_url: &str,
    id: &str,
    version: &str,
    trusted_key_hex: &str,
    install_root: &Path,
) -> Result<Vec<(String, String)>, String> {
    validate_extension_version(id, version)?;
    let client = MarketplaceClient::new(base_url)?;
    let _lock = InstallLock::acquire(install_root)?;
    let installed = installed_versions(install_root)?
        .into_iter()
        .map(|(id, version)| {
            semver::Version::parse(&version)
                .map(|version| (id, version))
                .map_err(|error| format!("Invalid installed package version: {error}"))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let mut visiting = HashSet::new();
    let mut planned = HashMap::new();
    let mut order = Vec::new();
    collect_package(
        &client,
        id,
        version,
        trusted_key_hex,
        &installed,
        &mut visiting,
        &mut planned,
        &mut order,
        0,
    )?;
    install_package_batch(&order, &planned, install_root)
}

const MAX_RESOLVED_EXTENSIONS: usize = 128;
const MAX_DEPENDENCY_DEPTH: usize = 32;

struct PlannedPackage {
    version: String,
    bytes: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
fn collect_package(
    client: &MarketplaceClient,
    id: &str,
    version: &str,
    trusted_key_hex: &str,
    installed: &HashMap<String, semver::Version>,
    visiting: &mut HashSet<String>,
    planned: &mut HashMap<String, PlannedPackage>,
    order: &mut Vec<String>,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err("Extension dependency graph is too deep".into());
    }
    if visiting.contains(id) {
        return Err(format!("Extension dependency cycle contains {id}"));
    }
    if planned
        .get(id)
        .is_some_and(|package| package.version == version)
    {
        return Ok(());
    }
    if planned.contains_key(id) {
        return Err(format!(
            "Dependency graph requests multiple versions of {id}"
        ));
    }
    if planned.len() >= MAX_RESOLVED_EXTENSIONS {
        return Err("Extension dependency graph exceeds 128 packages".into());
    }
    validate_extension_version(id, version)?;
    let package = client.download(id, version)?;
    verify_package_signature(&package, trusted_key_hex)?;
    let manifest = read_package_manifest(&package.bytes)?;
    if manifest.id != id || manifest.version != version {
        return Err(format!(
            "Downloaded package manifest does not match {id} {version}"
        ));
    }
    validate_manifest_compatibility(&manifest)?;

    visiting.insert(id.to_owned());
    let mut dependencies = manifest.dependencies.iter().collect::<Vec<_>>();
    dependencies.sort_by(|left, right| left.0.cmp(right.0));
    for (dependency_id, range) in dependencies {
        let requirement = semver::VersionReq::parse(range)
            .map_err(|_| format!("Invalid version range for dependency {dependency_id}"))?;
        if installed
            .get(dependency_id)
            .is_some_and(|installed_version| requirement.matches(installed_version))
        {
            continue;
        }
        let candidate = planned
            .get(dependency_id)
            .and_then(|package| semver::Version::parse(&package.version).ok())
            .filter(|candidate| requirement.matches(candidate));
        if let Some(candidate) = candidate {
            collect_package(
                client,
                dependency_id,
                &candidate.to_string(),
                trusted_key_hex,
                installed,
                visiting,
                planned,
                order,
                depth + 1,
            )?;
            continue;
        }
        let mut versions = client.versions(dependency_id)?;
        versions.sort_by(|left, right| right.cmp(left));
        let candidate = versions
            .into_iter()
            .find(|candidate| requirement.matches(candidate))
            .ok_or_else(|| format!("No published version of {dependency_id} satisfies {range}"))?;
        collect_package(
            client,
            dependency_id,
            &candidate.to_string(),
            trusted_key_hex,
            installed,
            visiting,
            planned,
            order,
            depth + 1,
        )?;
    }
    visiting.remove(id);
    planned.insert(
        id.to_owned(),
        PlannedPackage {
            version: manifest.version.clone(),
            bytes: package.bytes,
        },
    );
    order.push(id.to_owned());
    Ok(())
}

fn read_package_manifest(bytes: &[u8]) -> Result<PackageManifest, String> {
    let mut archive =
        ZipArchive::new(io::Cursor::new(bytes)).map_err(|_| "Package is not a ZIP archive")?;
    let mut file = archive
        .by_name(MANIFEST_NAME)
        .map_err(|_| format!("Package has no root {MANIFEST_NAME}"))?;
    if file.size() > MAX_MANIFEST_BYTES {
        return Err("Package manifest exceeds 64 KiB".into());
    }
    let mut data = Vec::new();
    file.by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    if data.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Package manifest exceeds 64 KiB".into());
    }
    serde_json::from_slice(&data).map_err(|error| format!("Invalid package manifest: {error}"))
}

fn validate_manifest_compatibility(manifest: &PackageManifest) -> Result<(), String> {
    if let Some(range) = &manifest.nortex {
        let requirement = semver::VersionReq::parse(range)
            .map_err(|_| "Package has an invalid NORTEX compatibility range")?;
        let ide_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|error| format!("IDE version is invalid: {error}"))?;
        if !requirement.matches(&ide_version) {
            return Err(format!(
                "Package {} requires NORTEX {range}, but this IDE is {ide_version}",
                manifest.id
            ));
        }
    }
    Ok(())
}

fn install_package_batch(
    order: &[String],
    planned: &HashMap<String, PlannedPackage>,
    install_root: &Path,
) -> Result<Vec<(String, String)>, String> {
    fs::create_dir_all(install_root).map_err(|error| error.to_string())?;
    let parent = install_root
        .parent()
        .ok_or_else(|| "Extension install directory has no parent".to_string())?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let backup_root = parent.join(format!(
        ".nortex-install-backup-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&backup_root).map_err(|error| error.to_string())?;
    let mut changed: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let mut installed = Vec::new();

    for id in order {
        let Some(package) = planned.get(id) else {
            let rollback = rollback_package_batch(&changed);
            let _ = fs::remove_dir(&backup_root);
            return match rollback {
                Ok(()) => Err(format!("Install plan is missing package {id}")),
                Err(error) => Err(format!(
                    "Install plan is missing package {id}; rollback failed: {error}"
                )),
            };
        };
        let destination = install_root.join(id);
        let backup = if destination.exists() {
            let metadata = match fs::symlink_metadata(&destination) {
                Ok(metadata) => metadata,
                Err(error) => {
                    let rollback = rollback_package_batch(&changed);
                    let _ = fs::remove_dir(&backup_root);
                    return match rollback {
                        Ok(()) => Err(error.to_string()),
                        Err(rollback_error) => {
                            Err(format!("{error}; rollback failed: {rollback_error}"))
                        }
                    };
                }
            };
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                let rollback = rollback_package_batch(&changed);
                let _ = fs::remove_dir(&backup_root);
                return match rollback {
                    Ok(()) => Err(format!(
                        "Existing install target for {id} is not a real directory"
                    )),
                    Err(error) => Err(format!(
                        "Existing install target for {id} is not a real directory; rollback failed: {error}"
                    )),
                };
            }
            let backup = backup_root.join(id);
            if let Err(error) = fs::rename(&destination, &backup) {
                let rollback = rollback_package_batch(&changed);
                let _ = fs::remove_dir(&backup_root);
                return match rollback {
                    Ok(()) => Err(error.to_string()),
                    Err(rollback_error) => {
                        Err(format!("{error}; rollback failed: {rollback_error}"))
                    }
                };
            }
            Some(backup)
        } else {
            None
        };

        match install_package(&package.bytes, id, &package.version, install_root) {
            Ok((installed_id, name)) => {
                changed.push((destination, backup));
                installed.push((installed_id, name));
            }
            Err(error) => {
                if let Some(backup) = &backup {
                    if let Err(restore_error) = fs::rename(backup, &destination) {
                        let rollback = rollback_package_batch(&changed);
                        return Err(match rollback {
                            Ok(()) => format!(
                                "{error}; restoring {id} failed: {restore_error}; backup kept at {}",
                                backup.display()
                            ),
                            Err(rollback_error) => format!(
                                "{error}; restoring {id} failed: {restore_error}; rollback failed: {rollback_error}; backup kept at {}",
                                backup.display()
                            ),
                        });
                    }
                }
                let rollback = rollback_package_batch(&changed);
                let _ = fs::remove_dir_all(&backup_root);
                return match rollback {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(format!(
                        "{error}; dependency install rollback failed: {rollback_error}"
                    )),
                };
            }
        }
    }

    fs::remove_dir_all(&backup_root).map_err(|error| {
        format!(
            "Extensions installed, but backup cleanup failed at {}: {error}",
            backup_root.display()
        )
    })?;
    Ok(installed)
}

fn rollback_package_batch(changed: &[(PathBuf, Option<PathBuf>)]) -> Result<(), String> {
    let mut errors = Vec::new();
    for (destination, backup) in changed.iter().rev() {
        if destination.exists()
            && let Err(error) = fs::remove_dir_all(destination)
        {
            errors.push(format!("remove {}: {error}", destination.display()));
            continue;
        }
        if let Some(backup) = backup
            && let Err(error) = fs::rename(backup, destination)
        {
            errors.push(format!(
                "restore {} to {}: {error}",
                backup.display(),
                destination.display()
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn verify_package_signature(
    package: &DownloadedPackage,
    trusted_key_hex: &str,
) -> Result<(), String> {
    let actual_digest = Sha256::digest(&package.bytes);
    if hex::encode(actual_digest) != package.digest {
        return Err("Downloaded package digest does not match".into());
    }
    let key_bytes = hex::decode(trusted_key_hex.trim())
        .map_err(|_| "Trusted signing key must be 64 hexadecimal characters".to_string())?;
    let key_bytes: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| "Trusted signing key must be 32 bytes".to_string())?;
    let key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| "Trusted signing key is invalid".to_string())?;
    let signature_bytes = hex::decode(&package.signature)
        .map_err(|_| "Marketplace signature is invalid".to_string())?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| "Marketplace signature has an invalid length".to_string())?;
    key.verify(&actual_digest, &signature)
        .map_err(|_| "Downloaded package signature is not trusted".to_string())?;
    Ok(())
}

fn validate_extension_version(id: &str, version: &str) -> Result<(), String> {
    validate_extension_id(id)?;
    let parsed = semver::Version::parse(version).map_err(|_| "Extension version is invalid")?;
    if parsed.to_string() != version || parsed.build != semver::BuildMetadata::EMPTY {
        return Err("Extension version is not canonical".into());
    }
    Ok(())
}

fn validate_extension_id(id: &str) -> Result<(), String> {
    if !(3..=64).contains(&id.len())
        || !id.as_bytes()[0].is_ascii_lowercase() && !id.as_bytes()[0].is_ascii_digit()
        || !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        || id.contains("..")
    {
        return Err("Extension ID is invalid".into());
    }
    Ok(())
}

fn safe_package_path(name: &str) -> bool {
    if name.is_empty()
        || name.len() > 255
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name.chars().any(char::is_control)
    {
        return false;
    }
    let name = name.strip_suffix('/').unwrap_or(name);
    !name.is_empty()
        && name.split('/').all(|segment| {
            if segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.ends_with('.')
                || segment.ends_with(' ')
            {
                return false;
            }
            let basename = segment
                .split('.')
                .next()
                .unwrap_or_default()
                .trim_end_matches(' ');
            !matches!(
                basename.to_uppercase().as_str(),
                "CON"
                    | "PRN"
                    | "AUX"
                    | "NUL"
                    | "COM1"
                    | "COM2"
                    | "COM3"
                    | "COM4"
                    | "COM5"
                    | "COM6"
                    | "COM7"
                    | "COM8"
                    | "COM9"
                    | "LPT1"
                    | "LPT2"
                    | "LPT3"
                    | "LPT4"
                    | "LPT5"
                    | "LPT6"
                    | "LPT7"
                    | "LPT8"
                    | "LPT9"
            )
        })
}

fn install_package(
    bytes: &[u8],
    expected_id: &str,
    expected_version: &str,
    install_root: &Path,
) -> Result<(String, String), String> {
    let mut archive =
        ZipArchive::new(io::Cursor::new(bytes)).map_err(|_| "Package is not a ZIP archive")?;
    if archive.is_empty() || archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Package has an invalid number of archive entries".into());
    }
    let mut total_size = 0u64;
    let mut paths = HashSet::new();
    let mut manifest_found = false;
    for index in 0..archive.len() {
        let file = archive
            .by_index_raw(index)
            .map_err(|_| "Package contains an unreadable archive entry")?;
        let name = file.name();
        if !safe_package_path(name) {
            return Err(format!("Package contains an unsafe path: {name:?}"));
        }
        if !paths.insert(name.to_lowercase()) {
            return Err("Package contains duplicate paths".into());
        }
        if matches!(file.unix_mode(), Some(mode) if mode & 0o170000 == 0o120000) {
            return Err("Package contains a symbolic link".into());
        }
        total_size = total_size.saturating_add(file.size());
        if total_size > MAX_UNCOMPRESSED_BYTES {
            return Err("Package exceeds the 256 MiB expanded size limit".into());
        }
        manifest_found |= name == MANIFEST_NAME;
    }
    if !manifest_found {
        return Err(format!("Package has no root {MANIFEST_NAME}"));
    }

    let mut manifest_file = archive
        .by_name(MANIFEST_NAME)
        .map_err(|_| format!("Package has no root {MANIFEST_NAME}"))?;
    if manifest_file.size() > MAX_MANIFEST_BYTES {
        return Err("Package manifest exceeds 64 KiB".into());
    }
    let mut manifest_bytes = Vec::new();
    manifest_file
        .by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut manifest_bytes)
        .map_err(|_| "Package manifest could not be read")?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Package manifest exceeds 64 KiB".into());
    }
    drop(manifest_file);
    let manifest: PackageManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| "Package manifest is invalid JSON")?;
    let parsed_version = semver::Version::parse(&manifest.version)
        .map_err(|_| "Package version is not valid semantic versioning")?;
    if manifest.id != expected_id
        || manifest.version != expected_version
        || manifest.name.trim().is_empty()
        || manifest.name.chars().count() > 100
        || manifest.description.chars().count() > 2000
        || parsed_version.to_string() != manifest.version
        || parsed_version.build != semver::BuildMetadata::EMPTY
    {
        return Err("Package manifest does not match the requested extension/version".into());
    }
    if manifest.permissions.len() > 16
        || manifest.permissions.iter().any(|permission| {
            !matches!(
                permission.as_str(),
                "network.client"
                    | "network.private"
                    | "filesystem.package.read"
                    | "filesystem.package.write"
            )
        })
    {
        return Err("Package requests an unsupported permission".into());
    }
    nortex_ide::plugins::protocol::PluginManifest::from_json(&manifest_bytes)
        .map_err(|error| format!("Package manifest rejected: {error}"))?;
    validate_package_requirements(&manifest, install_root)?;
    if let Some(entry) = &manifest.entry {
        if !safe_package_path(entry) || entry.ends_with('/') {
            return Err("Package manifest entry is not a safe file in the archive".into());
        }
        let entry_file = archive
            .by_name(entry)
            .map_err(|_| "Package manifest entry is missing from the archive")?;
        if entry_file.is_dir() {
            return Err("Package manifest entry must be a file".into());
        }
    }
    drop(archive);
    extract_atomically(bytes, expected_id, install_root)?;
    Ok((manifest.id, manifest.name))
}

fn validate_package_requirements(
    requested: &PackageManifest,
    install_root: &Path,
) -> Result<(), String> {
    let ide_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| format!("IDE version is invalid: {error}"))?;
    if let Some(range) = &requested.nortex {
        let requirement = semver::VersionReq::parse(range)
            .map_err(|_| "Package has an invalid NORTEX compatibility range")?;
        if !requirement.matches(&ide_version) {
            return Err(format!(
                "Package requires NORTEX {range}, but this IDE is {ide_version}"
            ));
        }
    }

    let mut installed = HashMap::<String, PackageManifest>::new();
    if install_root.exists() {
        for entry in fs::read_dir(install_root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let bytes = fs::read(entry.path().join(MANIFEST_NAME))
                .map_err(|error| format!("Cannot read installed extension manifest: {error}"))?;
            if bytes.len() as u64 > MAX_MANIFEST_BYTES {
                return Err("Installed extension manifest exceeds 64 KiB".into());
            }
            let manifest: PackageManifest = serde_json::from_slice(&bytes)
                .map_err(|error| format!("Installed extension manifest is invalid: {error}"))?;
            if manifest.id != entry.file_name().to_string_lossy() {
                return Err("Installed extension folder does not match its manifest ID".into());
            }
            installed.insert(manifest.id.clone(), manifest);
        }
    }
    installed.insert(requested.id.clone(), requested.clone());

    for manifest in installed.values() {
        for (dependency_id, range) in &manifest.dependencies {
            validate_extension_id(dependency_id)?;
            if dependency_id == &manifest.id {
                return Err(format!("Extension {} depends on itself", manifest.id));
            }
            let requirement = semver::VersionReq::parse(range)
                .map_err(|_| format!("Invalid version range for dependency {dependency_id}"))?;
            let dependency = installed.get(dependency_id).ok_or_else(|| {
                format!(
                    "Missing dependency {dependency_id} required by {}",
                    manifest.id
                )
            })?;
            let version = semver::Version::parse(&dependency.version).map_err(|error| {
                format!("Invalid installed version for dependency {dependency_id}: {error}")
            })?;
            if !requirement.matches(&version) {
                return Err(format!(
                    "Extension {} requires {dependency_id} {range}, but {version} is installed",
                    manifest.id
                ));
            }
        }
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for id in installed.keys() {
        if dependency_cycle(id, &installed, &mut visiting, &mut visited) {
            return Err("Installed extensions contain a dependency cycle".into());
        }
    }
    Ok(())
}

fn dependency_cycle(
    id: &str,
    installed: &HashMap<String, PackageManifest>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> bool {
    if visited.contains(id) {
        return false;
    }
    if !visiting.insert(id.to_owned()) {
        return true;
    }
    if let Some(manifest) = installed.get(id) {
        for dependency in manifest.dependencies.keys() {
            if dependency_cycle(dependency, installed, visiting, visited) {
                return true;
            }
        }
    }
    visiting.remove(id);
    visited.insert(id.to_owned());
    false
}

fn extract_atomically(bytes: &[u8], id: &str, install_root: &Path) -> Result<(), String> {
    fs::create_dir_all(install_root).map_err(|error| error.to_string())?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let staging = install_root.join(format!(".{id}.staging-{}-{nonce}", std::process::id()));
    let backup = install_root.join(format!(".{id}.backup-{}-{nonce}", std::process::id()));
    let destination = install_root.join(id);
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let extraction = (|| -> Result<(), String> {
        let mut archive =
            ZipArchive::new(io::Cursor::new(bytes)).map_err(|_| "Package is not a ZIP archive")?;
        let mut extracted = 0u64;
        for index in 0..archive.len() {
            let mut file = archive
                .by_index(index)
                .map_err(|_| "Package entry could not be decompressed")?;
            let name = file.name().to_owned();
            if !safe_package_path(&name) {
                return Err(format!("Package contains an unsafe path: {name:?}"));
            }
            let target = staging.join(PathBuf::from(&name));
            if file.is_dir() {
                fs::create_dir_all(target).map_err(|error| error.to_string())?;
                continue;
            }
            let parent = target
                .parent()
                .ok_or_else(|| "Package entry has no parent directory".to_string())?;
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            let remaining = MAX_UNCOMPRESSED_BYTES.saturating_sub(extracted);
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|error| error.to_string())?;
            let written = io::copy(&mut file.by_ref().take(remaining + 1), &mut output)
                .map_err(|error| error.to_string())?;
            if written > remaining || written != file.size() {
                return Err("Package expanded size does not match its archive metadata".into());
            }
            extracted += written;
            output.flush().map_err(|error| error.to_string())?;
        }
        Ok(())
    })();

    if let Err(error) = extraction {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let had_destination = match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => true,
        Ok(_) => {
            let _ = fs::remove_dir_all(&staging);
            return Err("Existing install target is not a regular directory".into());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error.to_string());
        }
    };
    if had_destination {
        fs::rename(&destination, &backup).map_err(|error| {
            let _ = fs::remove_dir_all(&staging);
            error.to_string()
        })?;
    }
    if let Err(error) = fs::rename(&staging, &destination) {
        if had_destination {
            if let Err(restore_error) = fs::rename(&backup, &destination) {
                let _ = fs::remove_dir_all(&staging);
                return Err(format!(
                    "Install failed ({error}); previous package could not be restored ({restore_error}) and remains at {}",
                    backup.display()
                ));
            }
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(error.to_string());
    }
    if had_destination {
        fs::remove_dir_all(backup).map_err(|error| {
            format!("Package installed but previous version cleanup failed: {error}")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::io::Cursor;
    use std::time::SystemTime;

    fn package(id: &str, version: &str, extra_path: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        let manifest = serde_json::json!({
            "id": id,
            "name": "Test Extension",
            "version": version,
            "entry": "main.nx"
        });
        writer.start_file(MANIFEST_NAME, options).unwrap();
        writer.write_all(manifest.to_string().as_bytes()).unwrap();
        writer.start_file("main.nx", options).unwrap();
        writer.write_all(version.as_bytes()).unwrap();
        if !extra_path.is_empty() {
            writer.start_file(extra_path, options).unwrap();
            writer.write_all(b"bad").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn package_with_requirements(
        id: &str,
        version: &str,
        nortex: Option<&str>,
        dependencies: HashMap<String, String>,
    ) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        let manifest = serde_json::json!({
            "id": id,
            "name": "Test Extension",
            "version": version,
            "entry": "main.nx",
            "nortex": nortex,
            "dependencies": dependencies
        });
        writer.start_file(MANIFEST_NAME, options).unwrap();
        writer.write_all(manifest.to_string().as_bytes()).unwrap();
        writer.start_file("main.nx", options).unwrap();
        writer.write_all(version.as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }

    /// Minimal loopback marketplace: one canned response per request path.
    struct MockMarketplace {
        url: String,
        key: SigningKey,
    }

    fn deps(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(id, range)| (id.to_string(), range.to_string()))
            .collect()
    }

    /// `catalog`: (id, version, dependencies). Packages are signed with `key`
    /// unless `forged` names `id@version`, which gets a foreign signature.
    fn mock_marketplace(
        catalog: &[(&str, &str, HashMap<String, String>)],
        forged: &[&str],
    ) -> MockMarketplace {
        use std::io::BufRead;
        use std::net::TcpListener;

        let key = SigningKey::from_bytes(&[11; 32]);
        let foreign = SigningKey::from_bytes(&[12; 32]);
        type Response = (Vec<(String, String)>, Vec<u8>);
        let mut routes: HashMap<String, Response> = HashMap::new();
        let mut versions: HashMap<&str, Vec<&str>> = HashMap::new();
        for (id, version, dependencies) in catalog {
            versions.entry(id).or_default().push(version);
            let bytes = package_with_requirements(id, version, None, dependencies.clone());
            let digest = Sha256::digest(&bytes);
            let signer = if forged.contains(&format!("{id}@{version}").as_str()) {
                &foreign
            } else {
                &key
            };
            routes.insert(
                format!("/api/v1/extensions/{id}/versions/{version}/download"),
                (
                    vec![
                        ("x-nortex-sha256".into(), hex::encode(digest)),
                        (
                            "x-nortex-signature".into(),
                            hex::encode(signer.sign(&digest).to_bytes()),
                        ),
                    ],
                    bytes,
                ),
            );
        }
        for (id, list) in versions {
            let body = serde_json::json!({
                "versions": list.iter().map(|v| serde_json::json!({ "version": v })).collect::<Vec<_>>()
            });
            routes.insert(
                format!("/api/v1/extensions/{id}"),
                (Vec::new(), body.to_string().into_bytes()),
            );
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = io::BufReader::new(&stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let path = request_line.split_whitespace().nth(1).unwrap_or("/");
                let mut stream = &stream;
                match routes.get(path) {
                    Some((headers, body)) => {
                        let mut head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                            body.len()
                        );
                        for (name, value) in headers {
                            head.push_str(&format!("{name}: {value}\r\n"));
                        }
                        head.push_str("\r\n");
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(body);
                    }
                    None => {
                        let _ = stream.write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                }
            }
        });
        MockMarketplace { url, key }
    }

    impl MockMarketplace {
        fn trusted_key(&self) -> String {
            hex::encode(self.key.verifying_key().to_bytes())
        }

        fn install(
            &self,
            id: &str,
            version: &str,
            root: &Path,
        ) -> Result<Vec<(String, String)>, String> {
            download_and_install_with_dependencies(
                &self.url,
                id,
                version,
                &self.trusted_key(),
                root,
            )
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("nortex-market-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn installed_ids(root: &Path) -> Vec<String> {
        let mut ids: Vec<_> = installed_versions(root)
            .unwrap()
            .into_iter()
            .map(|(id, version)| format!("{id}@{version}"))
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn transitive_dependencies_resolve_to_the_newest_matching_version() {
        let market = mock_marketplace(
            &[
                ("app.ext", "1.0.0", deps(&[("lib.ext", "^1")])),
                ("lib.ext", "1.2.0", deps(&[("core.ext", ">=2, <3")])),
                ("core.ext", "2.0.0", HashMap::new()),
                ("core.ext", "2.4.0", HashMap::new()),
                ("core.ext", "3.0.0", HashMap::new()),
            ],
            &[],
        );
        let root = temp_root("transitive");
        let installed = market.install("app.ext", "1.0.0", &root).unwrap();
        let order: Vec<_> = installed.iter().map(|(id, _)| id.as_str()).collect();
        let listing = installed_ids(&root);
        let _ = fs::remove_dir_all(&root);
        assert_eq!(order, ["core.ext", "lib.ext", "app.ext"]);
        assert_eq!(
            listing,
            ["app.ext@1.0.0", "core.ext@2.4.0", "lib.ext@1.2.0"]
        );
    }

    #[test]
    fn installed_package_with_the_wrong_version_is_not_accepted_as_a_dependency() {
        let market = mock_marketplace(
            &[
                ("app.ext", "1.0.0", deps(&[("core.ext", "^2")])),
                ("core.ext", "2.1.0", HashMap::new()),
            ],
            &[],
        );
        let root = temp_root("wrong-version");
        fs::create_dir_all(root.join("core.ext")).unwrap();
        fs::write(
            root.join("core.ext").join(MANIFEST_NAME),
            r#"{"id":"core.ext","name":"Core","version":"1.0.0"}"#,
        )
        .unwrap();
        market.install("app.ext", "1.0.0", &root).unwrap();
        let listing = installed_ids(&root);
        let _ = fs::remove_dir_all(&root);
        assert_eq!(listing, ["app.ext@1.0.0", "core.ext@2.1.0"]);
    }

    #[test]
    fn unsatisfiable_missing_and_cyclic_dependencies_install_nothing() {
        let market = mock_marketplace(
            &[
                ("needs-missing.ext", "1.0.0", deps(&[("ghost.ext", "^1")])),
                ("needs-new.ext", "1.0.0", deps(&[("core.ext", "^5")])),
                ("core.ext", "2.0.0", HashMap::new()),
                ("ping.ext", "1.0.0", deps(&[("pong.ext", "^1")])),
                ("pong.ext", "1.0.0", deps(&[("ping.ext", "^1")])),
            ],
            &[],
        );
        let root = temp_root("failures");
        let missing = market.install("needs-missing.ext", "1.0.0", &root);
        let incompatible = market.install("needs-new.ext", "1.0.0", &root);
        let cyclic = market.install("ping.ext", "1.0.0", &root);
        let leftovers = installed_ids(&root);
        let _ = fs::remove_dir_all(&root);
        assert!(missing.is_err());
        assert!(
            incompatible
                .unwrap_err()
                .contains("No published version of core.ext satisfies ^5")
        );
        assert!(cyclic.unwrap_err().contains("cycle"));
        assert!(leftovers.is_empty());
    }

    #[test]
    fn a_forged_dependency_signature_aborts_the_whole_install() {
        let market = mock_marketplace(
            &[
                ("app.ext", "1.0.0", deps(&[("lib.ext", "^1")])),
                ("lib.ext", "1.0.0", HashMap::new()),
            ],
            &["lib.ext@1.0.0"],
        );
        let root = temp_root("forged");
        let result = market.install("app.ext", "1.0.0", &root);
        let leftovers = installed_ids(&root);
        let _ = fs::remove_dir_all(&root);
        assert!(result.unwrap_err().contains("signature"));
        assert!(leftovers.is_empty());
    }

    #[test]
    fn conflicting_version_requirements_in_one_graph_are_rejected() {
        let market = mock_marketplace(
            &[
                (
                    "app.ext",
                    "1.0.0",
                    deps(&[("a.ext", "^1"), ("b.ext", "^1")]),
                ),
                ("a.ext", "1.0.0", deps(&[("core.ext", "^1")])),
                ("b.ext", "1.0.0", deps(&[("core.ext", "^2")])),
                ("core.ext", "1.0.0", HashMap::new()),
                ("core.ext", "2.0.0", HashMap::new()),
            ],
            &[],
        );
        let root = temp_root("conflict");
        let result = market.install("app.ext", "1.0.0", &root);
        let leftovers = installed_ids(&root);
        let _ = fs::remove_dir_all(&root);
        assert!(result.unwrap_err().contains("multiple versions"));
        assert!(leftovers.is_empty());
    }

    #[test]
    fn a_second_installation_is_refused_while_one_is_running() {
        let root = temp_root("lock").join("installed");
        let first = InstallLock::acquire(&root).unwrap();
        let second = InstallLock::acquire(&root);
        drop(first);
        let after_release = InstallLock::acquire(&root);
        let ok = after_release.is_ok();
        drop(after_release);
        let _ = fs::remove_dir_all(root.parent().unwrap());
        assert!(second.is_err());
        assert!(ok);
    }

    #[test]
    fn remote_marketplaces_require_https() {
        assert!(MarketplaceClient::new("http://127.0.0.1:8787").is_ok());
        assert!(MarketplaceClient::new("http://localhost:8787").is_ok());
        assert!(MarketplaceClient::new("https://market.example").is_ok());
        assert!(MarketplaceClient::new("http://market.example").is_err());
    }

    #[test]
    fn marketplace_url_rejects_embedded_credentials_and_paths() {
        assert!(MarketplaceClient::new("https://user:pass@market.example").is_err());
        assert!(MarketplaceClient::new("https://market.example/api").is_err());
    }

    #[test]
    fn package_paths_reject_traversal_and_windows_devices() {
        assert!(safe_package_path("src/main.nx"));
        assert!(!safe_package_path("../outside"));
        assert!(!safe_package_path("NUL.txt"));
        assert!(!safe_package_path("folder/COM1/file"));
    }

    #[test]
    fn package_requires_matching_digest_and_pinned_signature() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let bytes = b"signed package".to_vec();
        let digest = Sha256::digest(&bytes);
        let package = DownloadedPackage {
            bytes,
            digest: hex::encode(digest),
            signature: hex::encode(signing_key.sign(&digest).to_bytes()),
        };
        let trusted_key = hex::encode(signing_key.verifying_key().to_bytes());
        assert!(verify_package_signature(&package, &trusted_key).is_ok());

        let mut tampered = package;
        tampered.bytes.push(0);
        assert!(verify_package_signature(&tampered, &trusted_key).is_err());
    }

    #[test]
    fn installation_checks_manifest_and_rejects_unsafe_paths() {
        let root = std::env::temp_dir().join(format!(
            "nortex-marketplace-install-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let installed = install_package(
            &package("test.ext", "1.0.0", ""),
            "test.ext",
            "1.0.0",
            &root,
        )
        .unwrap();
        assert_eq!(installed, ("test.ext".into(), "Test Extension".into()));
        assert_eq!(
            fs::read(root.join("test.ext").join("main.nx")).unwrap(),
            b"1.0.0"
        );
        install_package(
            &package("test.ext", "2.0.0", ""),
            "test.ext",
            "2.0.0",
            &root,
        )
        .unwrap();
        assert_eq!(
            fs::read(root.join("test.ext").join("main.nx")).unwrap(),
            b"2.0.0"
        );
        assert!(
            install_package(
                &package("test.ext", "1.0.0", "../outside"),
                "test.ext",
                "1.0.0",
                &root
            )
            .is_err()
        );
        assert!(!root.join("outside").exists());
        assert_eq!(
            fs::read(root.join("test.ext").join("main.nx")).unwrap(),
            b"2.0.0"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installation_checks_compatibility_dependencies_and_cycles() {
        let root = std::env::temp_dir().join(format!(
            "nortex-marketplace-deps-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let empty = HashMap::new();
        install_package(
            &package_with_requirements("helper.ext", "1.2.0", None, empty.clone()),
            "helper.ext",
            "1.2.0",
            &root,
        )
        .unwrap();

        let mut dependencies = HashMap::new();
        dependencies.insert("helper.ext".into(), "^1.0".into());
        install_package(
            &package_with_requirements("consumer.ext", "1.0.0", None, dependencies),
            "consumer.ext",
            "1.0.0",
            &root,
        )
        .unwrap();

        let mut missing = HashMap::new();
        missing.insert("absent.ext".into(), "*".into());
        assert!(
            install_package(
                &package_with_requirements("missing.ext", "1.0.0", None, missing),
                "missing.ext",
                "1.0.0",
                &root,
            )
            .unwrap_err()
            .contains("Missing dependency")
        );
        assert!(
            install_package(
                &package_with_requirements(
                    "newer-ide.ext",
                    "1.0.0",
                    Some(">=999.0"),
                    empty.clone()
                ),
                "newer-ide.ext",
                "1.0.0",
                &root,
            )
            .unwrap_err()
            .contains("requires NORTEX")
        );

        let mut cycle = HashMap::new();
        cycle.insert("consumer.ext".into(), "^1.0".into());
        assert!(
            install_package(
                &package_with_requirements("helper.ext", "1.3.0", None, cycle),
                "helper.ext",
                "1.3.0",
                &root,
            )
            .unwrap_err()
            .contains("dependency cycle")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dependency_batch_installs_in_order_and_rolls_back_on_failure() {
        let root = std::env::temp_dir().join(format!(
            "nortex-marketplace-batch-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut consumer_dependencies = HashMap::new();
        consumer_dependencies.insert("helper.ext".into(), "^1.0".into());
        let mut valid_plan = HashMap::new();
        valid_plan.insert(
            "helper.ext".into(),
            PlannedPackage {
                version: "1.0.0".into(),
                bytes: package_with_requirements("helper.ext", "1.0.0", None, HashMap::new()),
            },
        );
        valid_plan.insert(
            "consumer.ext".into(),
            PlannedPackage {
                version: "1.0.0".into(),
                bytes: package_with_requirements(
                    "consumer.ext",
                    "1.0.0",
                    None,
                    consumer_dependencies,
                ),
            },
        );
        let installed = install_package_batch(
            &["helper.ext".into(), "consumer.ext".into()],
            &valid_plan,
            &root,
        )
        .unwrap();
        assert_eq!(
            installed
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            ["helper.ext", "consumer.ext"]
        );

        let failed_root = root.with_extension("rollback");
        let mut helper_dependencies = HashMap::new();
        helper_dependencies.insert("absent.ext".into(), "^1.0".into());
        let mut planned = HashMap::new();
        planned.insert(
            "helper.ext".into(),
            PlannedPackage {
                version: "1.0.0".into(),
                bytes: package_with_requirements("helper.ext", "1.0.0", None, HashMap::new()),
            },
        );
        planned.insert(
            "consumer.ext".into(),
            PlannedPackage {
                version: "1.0.0".into(),
                bytes: package_with_requirements(
                    "consumer.ext",
                    "1.0.0",
                    None,
                    helper_dependencies,
                ),
            },
        );

        assert!(
            install_package_batch(
                &["helper.ext".into(), "consumer.ext".into()],
                &planned,
                &failed_root,
            )
            .unwrap_err()
            .contains("Missing dependency")
        );
        assert!(!failed_root.join("helper.ext").exists());
        assert!(!failed_root.join("consumer.ext").exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(failed_root).unwrap();
    }
}
