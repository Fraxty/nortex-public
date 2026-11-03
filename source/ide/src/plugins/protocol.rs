//! Wire format, API versions and the allow-list of the plugin IPC protocol.
//!
//! One JSON object per line, UTF-8, at most [`MAX_FRAME_BYTES`] bytes per line.

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::sandbox::normalize_extension_id;

/// Version of the framing and the request/response rules below.
pub const PROTOCOL_VERSION: u32 = 1;
/// Version of the IDE API (methods and their parameters). Minor versions only add
/// methods; a major change may remove or change them.
pub const IDE_API_VERSION: Version = Version::new(1, 2, 0);
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_IDE_PERMISSIONS: usize = 16;

pub const PERM_LOG: &str = "ide.log";
pub const PERM_DOCUMENT_READ: &str = "ide.document.read";
pub const PERM_PROJECT_READ: &str = "ide.project.read";
pub const PERM_DIAGNOSTICS_READ: &str = "ide.diagnostics.read";
pub const PERM_STATUS_WRITE: &str = "ide.status.write";

pub const PERM_DOCUMENT_WRITE: &str = "ide.document.write";
pub const PERM_FILE_READ: &str = "ide.file.read";
pub const PERM_FILE_WRITE: &str = "ide.file.write";
pub const PERM_NOTIFICATION: &str = "ide.notification";
pub const PERM_SEARCH: &str = "ide.search";
pub const PERM_GIT_READ: &str = "ide.git.read";
pub const PERM_THEME_READ: &str = "ide.theme.read";
pub const PERM_ARCHIVE_WRITE: &str = "ide.archive.write";
pub const PERM_DOCUMENT_EXPORT: &str = "ide.document.export";

const KNOWN_PERMISSIONS: [&str; 14] = [
    PERM_LOG,
    PERM_DOCUMENT_READ,
    PERM_PROJECT_READ,
    PERM_DIAGNOSTICS_READ,
    PERM_STATUS_WRITE,
    PERM_DOCUMENT_WRITE,
    PERM_FILE_READ,
    PERM_FILE_WRITE,
    PERM_NOTIFICATION,
    PERM_SEARCH,
    PERM_GIT_READ,
    PERM_THEME_READ,
    PERM_ARCHIVE_WRITE,
    PERM_DOCUMENT_EXPORT,
];

/// Methods a plugin may call, the IDE-API permission each one needs and the API
/// version that introduced it.
pub const API_METHODS: [ApiMethod; 14] = [
    ApiMethod::new("log", PERM_LOG, 1, 0),
    ApiMethod::new("document.active", PERM_DOCUMENT_READ, 1, 0),
    ApiMethod::new("project.files", PERM_PROJECT_READ, 1, 0),
    ApiMethod::new("diagnostics.active", PERM_DIAGNOSTICS_READ, 1, 0),
    ApiMethod::new("status.set", PERM_STATUS_WRITE, 1, 0),
    ApiMethod::new("document.replace", PERM_DOCUMENT_WRITE, 1, 1),
    ApiMethod::new("file.read", PERM_FILE_READ, 1, 1),
    ApiMethod::new("file.write", PERM_FILE_WRITE, 1, 1),
    ApiMethod::new("notification.show", PERM_NOTIFICATION, 1, 1),
    ApiMethod::new("search.text", PERM_SEARCH, 1, 1),
    ApiMethod::new("git.status", PERM_GIT_READ, 1, 1),
    ApiMethod::new("theme.current", PERM_THEME_READ, 1, 1),
    ApiMethod::new("archive.create", PERM_ARCHIVE_WRITE, 1, 2),
    ApiMethod::new("document.export_pdf", PERM_DOCUMENT_EXPORT, 1, 2),
];

pub const METHOD_INITIALIZE: &str = "initialize";
pub const METHOD_SHUTDOWN: &str = "shutdown";
/// Host-to-plugin request that runs one action the plugin declared in its
/// `initialize` result (`"actions": [{"id": "...", "title": "..."}]`).
pub const METHOD_ACTION_INVOKE: &str = "action.invoke";
/// Upper bound of actions one plugin may declare.
pub const MAX_ACTIONS: usize = 16;
pub const MAX_ACTION_ID_CHARS: usize = 48;
pub const MAX_ACTION_TITLE_CHARS: usize = 60;

#[derive(Debug, Clone, Copy)]
pub struct ApiMethod {
    pub name: &'static str,
    pub permission: &'static str,
    pub since: (u64, u64),
}

impl ApiMethod {
    const fn new(name: &'static str, permission: &'static str, major: u64, minor: u64) -> Self {
        Self {
            name,
            permission,
            since: (major, minor),
        }
    }

    pub fn find(name: &str) -> Option<&'static ApiMethod> {
        API_METHODS.iter().find(|method| method.name == name)
    }

    /// False when the running IDE API is older than the one that added the method.
    pub fn available(&self) -> bool {
        (IDE_API_VERSION.major, IDE_API_VERSION.minor) >= self.since
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Request,
    Response,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

/// Error codes returned to plugins.
pub mod code {
    pub const UNKNOWN_METHOD: &str = "unknown_method";
    pub const PERMISSION_DENIED: &str = "permission_denied";
    pub const INVALID_PARAMS: &str = "invalid_params";
    pub const NOT_INITIALIZED: &str = "not_initialized";
    pub const UNAVAILABLE: &str = "unavailable";
    pub const RATE_LIMITED: &str = "rate_limited";
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub protocol: u32,
    pub id: u64,
    #[serde(rename = "type")]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameError(pub String);

impl Frame {
    pub fn request(id: u64, method: &str, params: Value) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            id,
            kind: Kind::Request,
            method: Some(method.to_owned()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    pub fn success(id: u64, result: Value) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            id,
            kind: Kind::Response,
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(id: u64, code: &str, message: &str) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            id,
            kind: Kind::Response,
            method: None,
            params: None,
            result: None,
            error: Some(ErrorBody {
                code: code.to_owned(),
                message: message.to_owned(),
            }),
        }
    }

    /// Parses and validates one line (without the trailing newline).
    pub fn parse(line: &[u8]) -> Result<Self, FrameError> {
        if line.len() > MAX_FRAME_BYTES {
            return Err(FrameError("frame exceeds the size limit".into()));
        }
        let frame: Frame = serde_json::from_slice(line)
            .map_err(|error| FrameError(format!("invalid frame: {error}")))?;
        frame.validate()?;
        Ok(frame)
    }

    fn validate(&self) -> Result<(), FrameError> {
        if self.protocol != PROTOCOL_VERSION {
            return Err(FrameError(format!(
                "unsupported protocol version {} (supported: {PROTOCOL_VERSION})",
                self.protocol
            )));
        }
        match self.kind {
            Kind::Request => {
                let method = self.method.as_deref().unwrap_or_default();
                if method.is_empty()
                    || method.len() > 64
                    || self.result.is_some()
                    || self.error.is_some()
                {
                    return Err(FrameError("malformed request".into()));
                }
            }
            Kind::Response => {
                if self.method.is_some()
                    || self.params.is_some()
                    || self.result.is_some() == self.error.is_some()
                {
                    return Err(FrameError(
                        "a response needs exactly one of result and error".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn to_line(&self) -> Vec<u8> {
        let mut line = serde_json::to_vec(self).unwrap_or_default();
        line.push(b'\n');
        line
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityError {
    InvalidRequirement(String),
    Incompatible { required: String },
}

impl std::fmt::Display for CompatibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequirement(text) => write!(f, "invalid IDE API requirement {text:?}"),
            Self::Incompatible { required } => write!(
                f,
                "plugin requires IDE API {required}, this IDE provides {IDE_API_VERSION}"
            ),
        }
    }
}

/// Checks a semver requirement (for example `>=1.0, <2`) against [`IDE_API_VERSION`].
/// The requirement carries both the minimum and the maximum supported version.
pub fn check_compatibility(requirement: &str) -> Result<(), CompatibilityError> {
    let parsed = VersionReq::parse(requirement)
        .map_err(|_| CompatibilityError::InvalidRequirement(requirement.to_owned()))?;
    if parsed.matches(&IDE_API_VERSION) {
        Ok(())
    } else {
        Err(CompatibilityError::Incompatible {
            required: requirement.to_owned(),
        })
    }
}

/// Plugin metadata the host needs; read from `nortex-extension.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub ide_api: String,
    pub ide_permissions: Vec<String>,
}

#[derive(Deserialize)]
struct RawManifest {
    id: String,
    #[serde(default)]
    ide_api: Option<String>,
    #[serde(default)]
    ide_permissions: Vec<String>,
}

impl PluginManifest {
    /// `Ok(None)`: the package is a plain executable without IDE API (batch runner).
    pub fn from_json(bytes: &[u8]) -> Result<Option<Self>, String> {
        let raw: RawManifest =
            serde_json::from_slice(bytes).map_err(|error| format!("invalid manifest: {error}"))?;
        let Some(ide_api) = raw.ide_api else {
            if raw.ide_permissions.is_empty() {
                return Ok(None);
            }
            return Err("ide_permissions require an ide_api requirement".into());
        };
        check_compatibility(&ide_api).map_err(|error| error.to_string())?;
        let id = normalize_extension_id(&raw.id).map_err(|error| error.to_string())?;
        validate_ide_permissions(&raw.ide_permissions)?;
        Ok(Some(Self {
            id,
            ide_api,
            ide_permissions: raw.ide_permissions,
        }))
    }
}

pub fn validate_ide_permissions(names: &[String]) -> Result<(), String> {
    if names.len() > MAX_IDE_PERMISSIONS {
        return Err("manifest requests too many IDE permissions".into());
    }
    for (index, name) in names.iter().enumerate() {
        if !KNOWN_PERMISSIONS.contains(&name.as_str()) {
            return Err(format!("unknown IDE permission: {name}"));
        }
        if names[..index].contains(name) {
            return Err(format!("duplicate IDE permission: {name}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frames_round_trip_and_reject_malformed_input() {
        let request = Frame::request(7, "log", json!({"level": "info"}));
        let line = request.to_line();
        assert_eq!(Frame::parse(&line[..line.len() - 1]).unwrap(), request);

        for bad in [
            &br#"{"protocol":1,"id":1,"type":"request"}"#[..],
            br#"{"protocol":2,"id":1,"type":"request","method":"log"}"#,
            br#"{"protocol":1,"id":1,"type":"response"}"#,
            br#"{"protocol":1,"id":1,"type":"response","result":1,"error":{"code":"x","message":"y"}}"#,
            br#"{"protocol":1,"id":1,"type":"request","method":"log","extra":true}"#,
            br#"{"protocol":1,"id":"1","type":"request","method":"log"}"#,
            b"not json",
            b"",
        ] {
            assert!(Frame::parse(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
        assert!(Frame::parse(&vec![b' '; MAX_FRAME_BYTES + 1]).is_err());
    }

    #[test]
    fn compatibility_uses_minimum_and_maximum_of_the_requirement() {
        assert!(check_compatibility(">=1.0, <2").is_ok());
        assert!(check_compatibility("^1").is_ok());
        assert!(matches!(
            check_compatibility(">=1.3"),
            Err(CompatibilityError::Incompatible { .. })
        ));
        assert!(matches!(
            check_compatibility("<1"),
            Err(CompatibilityError::Incompatible { .. })
        ));
        assert!(matches!(
            check_compatibility("2"),
            Err(CompatibilityError::Incompatible { .. })
        ));
        assert!(matches!(
            check_compatibility("banana"),
            Err(CompatibilityError::InvalidRequirement(_))
        ));
    }

    #[test]
    fn manifest_metadata_is_validated() {
        let plain = br#"{"id":"demo.tool","entry":"a.exe"}"#;
        assert_eq!(PluginManifest::from_json(plain).unwrap(), None);

        let hosted = br#"{"id":"Demo.Tool","ide_api":"^1","ide_permissions":["ide.log"]}"#;
        let manifest = PluginManifest::from_json(hosted).unwrap().unwrap();
        assert_eq!(manifest.id, "demo.tool");
        assert_eq!(manifest.ide_permissions, ["ide.log"]);

        for bad in [
            &br#"{"id":"demo","ide_api":"^1","ide_permissions":["ide.everything"]}"#[..],
            br#"{"id":"demo","ide_api":"^1","ide_permissions":["ide.log","ide.log"]}"#,
            br#"{"id":"demo","ide_api":"^2"}"#,
            br#"{"id":"demo","ide_api":"nonsense"}"#,
            br#"{"id":"demo","ide_permissions":["ide.log"]}"#,
            br#"{"id":"../x","ide_api":"^1"}"#,
            br#"{"ide_api":"^1"}"#,
            b"[1]",
        ] {
            assert!(
                PluginManifest::from_json(bad).is_err(),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        let many = format!(
            r#"{{"id":"demo","ide_api":"^1","ide_permissions":[{}]}}"#,
            vec![r#""ide.log""#; MAX_IDE_PERMISSIONS + 1].join(",")
        );
        assert!(PluginManifest::from_json(many.as_bytes()).is_err());
    }

    #[test]
    fn every_method_requires_a_known_permission() {
        for method in &API_METHODS {
            assert!(KNOWN_PERMISSIONS.contains(&method.permission));
            assert!(method.available());
            assert_eq!(ApiMethod::find(method.name).unwrap().name, method.name);
        }
        assert!(ApiMethod::find("fs.read").is_none());
    }
}
