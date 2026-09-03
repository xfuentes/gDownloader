use std::sync::Arc;

use crate::jd::JdApi;

const INTERFACE: &str =
    "org.jdownloader.gui.views.linkgrabber.addlinksdialog.LinkgrabberSettings";
const VARIOUS_PACKAGE_ENABLED_KEY: &str = "VariousPackageEnabled";

#[derive(Clone)]
/// Accessor for JDownloader's LinkgrabberSettings.
pub struct LinkgrabberSettings {
    api: Arc<JdApi>,
}

impl LinkgrabberSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn get_various_package_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, VARIOUS_PACKAGE_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn set_various_package_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, VARIOUS_PACKAGE_ENABLED_KEY, &serde_json::Value::Bool(value))
    }
}
