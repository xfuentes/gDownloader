use std::sync::Arc;

use crate::jd::JdApi;

const INTERFACE: &str = "org.jdownloader.controlling.filter.LinkFilterSettings";
const LINK_FILTER_ENABLED_KEY: &str = "LinkFilterEnabled";

#[derive(Clone)]
/// Accessor for JDownloader's global LinkFilterSettings.
pub struct LinkFilterSettings {
    api: Arc<JdApi>,
}

impl LinkFilterSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn get_link_filter_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, LINK_FILTER_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn set_link_filter_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, LINK_FILTER_ENABLED_KEY, &serde_json::Value::Bool(value))
    }
}
