use std::sync::Arc;

use crate::jd::JdApi;
use serde_json::Value;

const INTERFACE: &str = "org.jdownloader.settings.GraphicalUserInterfaceSettings";
const CONFIG_VIEW_KEY: &str = "ConfigViewVisible";
const CLIPBOARD_MONITORED_KEY: &str = "ClipboardMonitored";

#[derive(Clone)]
/// Accessor for JDownloader's GraphicalUserInterfaceSettings.
pub struct GraphicalUserInterfaceSettings {
    api: Arc<JdApi>,
}

impl GraphicalUserInterfaceSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    /// Returns true when the JDownloader internal API is reachable.
    pub fn is_ready(&self) -> bool {
        self.api.is_ready()
    }

    /// Returns whether the configuration view is visible in the main tab bar.
    pub fn get_config_view_visible(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, CONFIG_VIEW_KEY)?
            .as_bool()
            .unwrap_or(false))
    }

    /// Sets whether the configuration view is visible in the main tab bar.
    pub fn set_config_view_visible(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, CONFIG_VIEW_KEY, &Value::Bool(value))
    }

    /// Returns whether the clipboard is being monitored.
    pub fn get_clipboard_monitored(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, CLIPBOARD_MONITORED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    /// Sets whether the clipboard is being monitored.
    pub fn set_clipboard_monitored(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, CLIPBOARD_MONITORED_KEY, &Value::Bool(value))
    }

    /// Reads an arbitrary boolean flag on `GraphicalUserInterfaceSettings` by
    /// its raw config key (e.g. the Downloads/Linkgrabber properties panel's
    /// per-field visibility flags).
    pub fn get_flag(&self, key: &str, default: bool) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, key)?
            .as_bool()
            .unwrap_or(default))
    }

    /// Writes an arbitrary boolean flag on `GraphicalUserInterfaceSettings`.
    pub fn set_flag(&self, key: &str, value: bool) -> anyhow::Result<bool> {
        self.api.set_config(INTERFACE, key, &Value::Bool(value))
    }
}
