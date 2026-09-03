// gDownloader
// Copyright (c) 2026. Xavier Fuentes <xfuentes-dev@serviam.cc>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::sync::Arc;

use crate::jd::JdApi;

const INTERFACE: &str = "jd.controlling.reconnect.ReconnectConfig";
const AUTO_RECONNECT_ENABLED_KEY: &str = "AutoReconnectEnabled";
const ACTIVE_PLUGIN_ID_KEY: &str = "ActivePluginID";

/// JDownloader ships a "DummyRouterPlugin" as the reconnect plugin's default
/// (unconfigured) value — reconnect is only actually possible once the user
/// picks a real router/script, so toolbar reconnect controls should disable
/// themselves while this is still selected.
pub const DUMMY_ROUTER_PLUGIN_ID: &str = "DummyRouterPlugin";

#[derive(Clone)]
/// Accessor for JDownloader's `ReconnectConfig`.
pub struct ReconnectSettings {
    api: Arc<JdApi>,
}

impl ReconnectSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn set_auto_reconnect_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, AUTO_RECONNECT_ENABLED_KEY, &serde_json::Value::Bool(value))
    }

    /// The currently configured reconnect plugin/script id, e.g.
    /// `"DummyRouterPlugin"` when none has been set up yet.
    pub fn get_active_plugin_id(&self) -> anyhow::Result<String> {
        Ok(self
            .api
            .get_config(INTERFACE, ACTIVE_PLUGIN_ID_KEY)?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
}
