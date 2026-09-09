use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::jd::JdApi;

/// Mirrors `org.jdownloader.api.extensions.ExtensionQueryStorable`.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionQuery {
    pub installed: bool,
    pub enabled: bool,
    pub name: bool,
    pub icon_key: bool,
    pub description: bool,
    pub config_interface: bool,
    pub pattern: Option<String>,
}

impl ExtensionQuery {
    /// Requests every field the RemoteAPI can return.
    pub fn all() -> Self {
        Self {
            installed: true,
            enabled: true,
            name: true,
            icon_key: true,
            description: true,
            config_interface: true,
            pattern: None,
        }
    }
}

/// Mirrors `org.jdownloader.api.extensions.ExtensionAPIStorable`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStorable {
    pub id: String,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub enabled: bool,
    pub name: Option<String>,
    pub icon_key: Option<String>,
    pub description: Option<String>,
    /// The extension's own config interface FQN, if any (not currently
    /// surfaced in gDownloader's UI — extension-specific settings pages are
    /// out of scope for the Extension Manager).
    #[allow(dead_code)]
    pub config_interface: Option<String>,
}

/// Accessor for JDownloader's `extensions` RemoteAPI (the "Extension
/// Manager" / "Modules d'extensions").
#[derive(Clone)]
pub struct JdExtensions {
    api: Arc<JdApi>,
}

impl JdExtensions {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn is_ready(&self) -> bool {
        self.api.is_ready()
    }

    pub fn list(&self, query: &ExtensionQuery) -> Result<Vec<ExtensionStorable>> {
        let query_json = serde_json::to_string(query)?;
        let value = self.api.call("extensions/list", &[query_json.as_str()])?;
        Ok(serde_json::from_value(value)?)
    }

    pub fn set_enabled(&self, classname: &str, enabled: bool) -> Result<bool> {
        let flag = if enabled { "true" } else { "false" };
        let value = self.api.call("extensions/setEnabled", &[classname, flag])?;
        Ok(value.as_bool().unwrap_or(false))
    }

    /// Triggers installation of an optional extension. JDownloader performs
    /// the download/installation server-side; the extension becomes usable
    /// once installed (some extensions require a JDownloader restart).
    pub fn install(&self, id: &str) -> Result<bool> {
        let value = self.api.call("extensions/install", &[id])?;
        Ok(value.as_bool().unwrap_or(false))
    }
}
