use std::sync::Arc;

use crate::jd::JdApi;

const INTERFACE: &str =
    "org.jdownloader.gui.views.linkgrabber.addlinksdialog.LinkgrabberSettings";
const VARIOUS_PACKAGE_ENABLED_KEY: &str = "VariousPackageEnabled";
const AUTO_EXTRACTION_ENABLED_KEY: &str = "AutoExtractionEnabled";
const AUTO_FILL_ADD_LINKS_DIALOG_WITH_CLIPBOARD_CONTENT_ENABLED_KEY: &str =
    "AutoFillAddLinksDialogWithClipboardContentEnabled";
const ADD_LINKS_DIALOG_OVERWRITES_PACKAGIZER_RULES_ENABLED_KEY: &str =
    "AddLinksDialogOverwritesPackagizerRulesEnabled";
const LINKGRABBER_ADD_AT_TOP_KEY: &str = "LinkgrabberAddAtTop";
const LINKGRABBER_AUTO_CONFIRM_ENABLED_KEY: &str = "LinkgrabberAutoConfirmEnabled";
const LINKGRABBER_AUTO_START_ENABLED_KEY: &str = "LinkgrabberAutoStartEnabled";

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

    pub fn get_auto_extraction_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, AUTO_EXTRACTION_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn get_auto_fill_add_links_dialog_with_clipboard_content_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, AUTO_FILL_ADD_LINKS_DIALOG_WITH_CLIPBOARD_CONTENT_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn get_add_links_dialog_overwrites_packagizer_rules_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, ADD_LINKS_DIALOG_OVERWRITES_PACKAGIZER_RULES_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn get_linkgrabber_add_at_top(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, LINKGRABBER_ADD_AT_TOP_KEY)?
            .as_bool()
            .unwrap_or(false))
    }

    pub fn set_linkgrabber_add_at_top(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, LINKGRABBER_ADD_AT_TOP_KEY, &serde_json::Value::Bool(value))
    }

    pub fn get_linkgrabber_auto_confirm_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, LINKGRABBER_AUTO_CONFIRM_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(false))
    }

    pub fn set_linkgrabber_auto_confirm_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, LINKGRABBER_AUTO_CONFIRM_ENABLED_KEY, &serde_json::Value::Bool(value))
    }

    pub fn get_linkgrabber_auto_start_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, LINKGRABBER_AUTO_START_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn set_linkgrabber_auto_start_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, LINKGRABBER_AUTO_START_ENABLED_KEY, &serde_json::Value::Bool(value))
    }
}
