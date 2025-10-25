use super::base::Config;
use crate::agents::extension::PLATFORM_EXTENSIONS;
use crate::agents::ExtensionConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::warn;
use utoipa::ToSchema;

pub const DEFAULT_EXTENSION: &str = "developer";
pub const DEFAULT_EXTENSION_TIMEOUT: u64 = 300;
pub const DEFAULT_EXTENSION_DESCRIPTION: &str = "";
pub const DEFAULT_DISPLAY_NAME: &str = "Developer";
const EXTENSIONS_CONFIG_KEY: &str = "extensions";
const EXTENSION_GROUPS_CONFIG_KEY: &str = "extension_groups";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, ToSchema)]
pub enum ExtensionGroupState {
    Enabled,
    Disabled,
    Mixed,
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct ExtensionGroup {
    pub name: String,
    pub extension_keys: Vec<String>,
}

impl ExtensionGroup {
    /// Get the extension group name
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get the extension group key (normalized name for storage)
    pub fn key(&self) -> String {
        name_to_key(&self.name)
    }

    /// Get the extension keys
    pub fn extension_keys(&self) -> &[String] {
        &self.extension_keys
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct ExtensionEntry {
    pub enabled: bool,
    #[serde(flatten)]
    pub config: ExtensionConfig,
}

pub fn name_to_key(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn get_extensions_map() -> HashMap<String, ExtensionEntry> {
    let raw: Value = Config::global()
        .get_param::<Value>(EXTENSIONS_CONFIG_KEY)
        .unwrap_or_else(|err| {
            warn!(
                "Failed to load {}: {err}. Falling back to empty object.",
                EXTENSIONS_CONFIG_KEY
            );
            Value::Object(serde_json::Map::new())
        });

    let mut extensions_map: HashMap<String, ExtensionEntry> = match raw {
        Value::Object(obj) => {
            let mut m = HashMap::with_capacity(obj.len());
            for (k, mut v) in obj {
                if let Value::Object(ref mut inner) = v {
                    match inner.get("description") {
                        Some(Value::Null) | None => {
                            inner.insert("description".to_string(), Value::String(String::new()));
                        }
                        _ => {}
                    }
                }
                match serde_json::from_value::<ExtensionEntry>(v.clone()) {
                    Ok(entry) => {
                        m.insert(k, entry);
                    }
                    Err(err) => {
                        let bad_json = serde_json::to_string(&v).unwrap_or_else(|e| {
                            format!("<failed to serialize malformed value: {e}>")
                        });
                        warn!(
                            extension = %k,
                            error = %err,
                            bad_json = %bad_json,
                            "Skipping malformed extension"
                        );
                    }
                }
            }
            m
        }
        other => {
            warn!(
                "Expected object for {}, got {}. Using empty map.",
                EXTENSIONS_CONFIG_KEY, other
            );
            HashMap::new()
        }
    };

    if !extensions_map.is_empty() {
        for (name, def) in PLATFORM_EXTENSIONS.iter() {
            if !extensions_map.contains_key(*name) {
                extensions_map.insert(
                    name.to_string(),
                    ExtensionEntry {
                        config: ExtensionConfig::Platform {
                            name: def.name.to_string(),
                            description: def.description.to_string(),
                            bundled: Some(true),
                            available_tools: Vec::new(),
                        },
                        enabled: true,
                    },
                );
            }
        }
    }
    extensions_map
}

fn save_extensions_map(extensions: HashMap<String, ExtensionEntry>) {
    let config = Config::global();
    match serde_json::to_value(extensions) {
        Ok(value) => {
            if let Err(e) = config.set_param(EXTENSIONS_CONFIG_KEY, value) {
                tracing::debug!("Failed to save extensions config: {}", e);
            }
        }
        Err(e) => {
            tracing::debug!("Failed to serialize extensions: {}", e);
        }
    }
}

pub fn get_extension_by_name(name: &str) -> Option<ExtensionConfig> {
    let extensions = get_extensions_map();
    extensions
        .values()
        .find(|entry| entry.config.name() == name)
        .map(|entry| entry.config.clone())
}

pub fn set_extension(entry: ExtensionEntry) {
    let mut extensions = get_extensions_map();
    let key = entry.config.key();
    extensions.insert(key, entry);
    save_extensions_map(extensions);
}

pub fn remove_extension(key: &str) {
    let mut extensions = get_extensions_map();
    extensions.remove(key);
    save_extensions_map(extensions);
}

pub fn set_extension_enabled(key: &str, enabled: bool) {
    let mut extensions = get_extensions_map();
    if let Some(entry) = extensions.get_mut(key) {
        entry.enabled = enabled;
        save_extensions_map(extensions);
    }
}

pub fn get_all_extensions() -> Vec<ExtensionEntry> {
    let extensions = get_extensions_map();
    extensions.into_values().collect()
}

pub fn get_all_extension_names() -> Vec<String> {
    let extensions = get_extensions_map();
    extensions.keys().cloned().collect()
}

pub fn is_extension_enabled(key: &str) -> bool {
    let extensions = get_extensions_map();
    extensions.get(key).map(|e| e.enabled).unwrap_or(false)
}

pub fn get_enabled_extensions() -> Vec<ExtensionConfig> {
    get_all_extensions()
        .into_iter()
        .filter(|ext| ext.enabled)
        .map(|ext| ext.config)
        .collect()
}

fn get_extension_groups_map() -> HashMap<String, ExtensionGroup> {
    let raw: Value = Config::global()
        .get_param::<Value>(EXTENSION_GROUPS_CONFIG_KEY)
        .unwrap_or_else(|err| {
            warn!(
                "Failed to load {}: {err}. Falling back to empty object.",
                EXTENSION_GROUPS_CONFIG_KEY
            );
            Value::Object(serde_json::Map::new())
        });

    match raw {
        Value::Object(obj) => {
            let mut groups_map = HashMap::with_capacity(obj.len());
            for (k, v) in obj {
                match serde_json::from_value::<ExtensionGroup>(v.clone()) {
                    Ok(group) => {
                        groups_map.insert(k, group);
                    }
                    Err(err) => {
                        let bad_json = serde_json::to_string(&v).unwrap_or_else(|e| {
                            format!("<failed to serialize malformed value: {e}>")
                        });
                        warn!(
                            group = %k,
                            error = %err,
                            bad_json = %bad_json,
                            "Skipping malformed extension group"
                        );
                    }
                }
            }
            groups_map
        }
        other => {
            warn!(
                "Expected object for {}, got {}. Using empty map.",
                EXTENSION_GROUPS_CONFIG_KEY, other
            );
            HashMap::new()
        }
    }
}

fn save_extension_groups_map(groups: HashMap<String, ExtensionGroup>) {
    let config = Config::global();
    match serde_json::to_value(groups) {
        Ok(value) => {
            if let Err(e) = config.set_param(EXTENSION_GROUPS_CONFIG_KEY, value) {
                tracing::debug!("Failed to save extension groups config: {}", e);
            }
        }
        Err(e) => {
            tracing::debug!("Failed to serialize extension groups: {}", e);
        }
    }
}

pub fn get_all_extension_groups() -> Vec<ExtensionGroup> {
    get_extension_groups_map().into_values().collect()
}

pub fn get_extension_group_by_name(name: &str) -> Option<ExtensionGroup> {
    let key = name_to_key(name);
    get_extension_groups_map().get(&key).cloned()
}

pub fn set_extension_group(group: ExtensionGroup) {
    let mut groups = get_extension_groups_map();
    let key = group.key();
    groups.insert(key, group);
    save_extension_groups_map(groups);
}

pub fn remove_extension_group(key: &str) {
    let mut groups = get_extension_groups_map();
    groups.remove(key);
    save_extension_groups_map(groups);
}

pub fn get_extension_group_state(group_name: &str) -> Option<ExtensionGroupState> {
    let group = get_extension_group_by_name(group_name)?;
    let extensions = get_extensions_map();

    if group.extension_keys().is_empty() {
        return Some(ExtensionGroupState::Disabled);
    }

    let enabled_count = group
        .extension_keys()
        .iter()
        .filter(|key| extensions.get(*key).map(|ext| ext.enabled).unwrap_or(false))
        .count();

    let total_count = group.extension_keys().len();

    match (enabled_count, total_count) {
        (0, _) => Some(ExtensionGroupState::Disabled),
        (count, total) if count == total => Some(ExtensionGroupState::Enabled),
        _ => Some(ExtensionGroupState::Mixed),
    }
}

pub fn enable_extension_group(group_name: &str) -> Result<(), String> {
    let group = get_extension_group_by_name(group_name)
        .ok_or_else(|| format!("Extension group '{}' not found", group_name))?;

    let mut extensions = get_extensions_map();
    let mut modified = false;

    for key in group.extension_keys() {
        if let Some(entry) = extensions.get_mut(key) {
            if !entry.enabled {
                entry.enabled = true;
                modified = true;
            }
        }
    }

    if modified {
        save_extensions_map(extensions);
    }

    Ok(())
}

pub fn disable_extension_group(group_name: &str) -> Result<(), String> {
    let group = get_extension_group_by_name(group_name)
        .ok_or_else(|| format!("Extension group '{}' not found", group_name))?;

    let mut extensions = get_extensions_map();
    let mut modified = false;

    for key in group.extension_keys() {
        if let Some(entry) = extensions.get_mut(key) {
            if entry.enabled {
                entry.enabled = false;
                modified = true;
            }
        }
    }

    if modified {
        save_extensions_map(extensions);
    }

    Ok(())
}

pub fn get_all_extension_group_names() -> Vec<String> {
    get_extension_groups_map().keys().cloned().collect()
}

pub fn set_extension_group_enabled(key: &str, enabled: bool) {
    let mut groups = get_extension_groups_map();
    if let Some(group) = groups.get_mut(key) {
        // For extension groups, we need to enable/disable all extensions in the group
        let extension_keys = group.extension_keys().to_vec();
        let mut extensions = get_extensions_map();
        let mut modified = false;
        
        for ext_key in extension_keys {
            if let Some(entry) = extensions.get_mut(&ext_key) {
                if entry.enabled != enabled {
                    entry.enabled = enabled;
                    modified = true;
                }
            }
        }
        
        if modified {
            save_extensions_map(extensions);
        }
    }
}

pub fn is_extension_group_enabled(key: &str) -> bool {
    let groups = get_extension_groups_map();
    if let Some(group) = groups.get(key) {
        let extensions = get_extensions_map();
        // Check if all extensions in the group are enabled
        group.extension_keys().iter().all(|ext_key| {
            extensions.get(ext_key).map(|e| e.enabled).unwrap_or(false)
        })
    } else {
        false
    }
}
