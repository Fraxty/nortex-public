//! Plugins must be explicitly approved before the IDE may enable them.

pub mod archive;
pub mod bundle;
pub mod host;
pub mod pdf;
pub mod project_io;
pub mod protocol;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub trait IdePlugin {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginApproval {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginRecord {
    pub name: String,
    pub approval: PluginApproval,
    pub enabled: bool,
    pub sidebar_visible: bool,
}

impl Default for PluginRecord {
    fn default() -> Self {
        Self {
            name: String::new(),
            approval: PluginApproval::Pending,
            enabled: false,
            sidebar_visible: true,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginRegistry {
    plugins: HashMap<String, PluginRecord>,
}

impl PluginRegistry {
    pub fn register(&mut self, id: impl Into<String>, name: impl Into<String>) {
        let id = id.into();
        let name = name.into();
        self.plugins
            .entry(id)
            .and_modify(|plugin| plugin.name.clone_from(&name))
            .or_insert_with(|| PluginRecord {
                name,
                ..PluginRecord::default()
            });
    }

    pub fn approve(&mut self, id: &str) -> bool {
        if let Some(plugin) = self.plugins.get_mut(id) {
            plugin.approval = PluginApproval::Approved;
            plugin.enabled = true;
            true
        } else {
            false
        }
    }

    pub fn reject(&mut self, id: &str) -> bool {
        if let Some(plugin) = self.plugins.get_mut(id) {
            plugin.approval = PluginApproval::Rejected;
            plugin.enabled = false;
            true
        } else {
            false
        }
    }

    pub fn require_review(&mut self, id: &str) -> bool {
        let Some(plugin) = self.plugins.get_mut(id) else {
            return false;
        };
        plugin.approval = PluginApproval::Pending;
        plugin.enabled = false;
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        self.plugins.remove(id).is_some()
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.plugins
            .get(id)
            .is_some_and(|plugin| plugin.approval == PluginApproval::Approved && plugin.enabled)
    }

    pub fn get(&self, id: &str) -> Option<&PluginRecord> {
        self.plugins.get(id)
    }

    pub fn records(&self) -> impl Iterator<Item = (&str, &PluginRecord)> {
        self.plugins
            .iter()
            .map(|(id, plugin)| (id.as_str(), plugin))
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        let Some(plugin) = self.plugins.get_mut(id) else {
            return false;
        };
        if enabled && plugin.approval != PluginApproval::Approved {
            return false;
        }
        plugin.enabled = enabled;
        true
    }

    pub fn set_sidebar_visible(&mut self, id: &str, visible: bool) -> bool {
        let Some(plugin) = self.plugins.get_mut(id) else {
            return false;
        };
        plugin.sidebar_visible = visible;
        true
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        serde_json::from_str(&text).map_err(io::Error::other)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_is_disabled_until_explicit_approval() {
        let mut registry = PluginRegistry::default();
        registry.register("formatter.extra", "Extra Formatter");
        assert!(!registry.is_enabled("formatter.extra"));
        assert!(registry.approve("formatter.extra"));
        assert!(registry.is_enabled("formatter.extra"));
    }

    #[test]
    fn activation_and_sidebar_visibility_are_independent() {
        let mut registry = PluginRegistry::default();
        registry.register("formatter.extra", "Extra Formatter");
        assert!(!registry.set_enabled("formatter.extra", true));
        assert!(registry.approve("formatter.extra"));
        assert!(registry.set_sidebar_visible("formatter.extra", false));
        assert!(registry.is_enabled("formatter.extra"));
        assert!(!registry.get("formatter.extra").unwrap().sidebar_visible);
        assert!(registry.set_enabled("formatter.extra", false));
        assert!(!registry.is_enabled("formatter.extra"));
        assert!(!registry.get("formatter.extra").unwrap().sidebar_visible);
    }

    #[test]
    fn rediscovery_preserves_explicit_approval() {
        let mut registry = PluginRegistry::default();
        registry.register("formatter.extra", "Old Name");
        assert!(registry.approve("formatter.extra"));
        registry.register("formatter.extra", "Updated Name");
        assert!(registry.is_enabled("formatter.extra"));
        assert_eq!(
            registry.get("formatter.extra").unwrap().name,
            "Updated Name"
        );
    }

    #[test]
    fn updated_package_requires_approval_again() {
        let mut registry = PluginRegistry::default();
        registry.register("formatter.extra", "Extra Formatter");
        assert!(registry.approve("formatter.extra"));
        assert!(registry.require_review("formatter.extra"));
        assert!(!registry.is_enabled("formatter.extra"));
        assert_eq!(
            registry.get("formatter.extra").unwrap().approval,
            PluginApproval::Pending
        );
    }

    #[test]
    fn registry_round_trips_and_missing_file_starts_empty() {
        let path = std::env::temp_dir().join(format!(
            "nortex-plugins-{}-registry.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        assert!(PluginRegistry::load(&path).unwrap().plugins.is_empty());

        let mut registry = PluginRegistry::default();
        registry.register("formatter.extra", "Extra Formatter");
        assert!(registry.approve("formatter.extra"));
        assert!(registry.set_sidebar_visible("formatter.extra", false));
        registry.save(&path).unwrap();
        let loaded = PluginRegistry::load(&path).unwrap();
        assert!(loaded.is_enabled("formatter.extra"));
        assert!(!loaded.get("formatter.extra").unwrap().sidebar_visible);
        let mut updated = loaded;
        assert!(updated.set_sidebar_visible("formatter.extra", true));
        updated.save(&path).unwrap();
        assert!(
            PluginRegistry::load(&path)
                .unwrap()
                .get("formatter.extra")
                .unwrap()
                .sidebar_visible
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_registry_is_reported() {
        let path = std::env::temp_dir().join(format!(
            "nortex-plugins-{}-invalid.json",
            std::process::id()
        ));
        fs::write(&path, "{ invalid").unwrap();
        assert!(PluginRegistry::load(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
