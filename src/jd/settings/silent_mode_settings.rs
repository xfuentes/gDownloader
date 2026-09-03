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

const INTERFACE: &str = "org.jdownloader.settings.SilentModeSettings";
const MANUAL_ENABLED_KEY: &str = "ManualEnabled";

#[derive(Clone)]
/// Accessor for JDownloader's `SilentModeSettings`.
pub struct SilentModeSettings {
    api: Arc<JdApi>,
}

impl SilentModeSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    /// Whether silent mode was manually switched on (the toolbar toggle's
    /// state — JDownloader also has automatic silent-mode triggers, which
    /// this doesn't reflect).
    pub fn get_manual_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, MANUAL_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(false))
    }

    pub fn set_manual_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.api
            .set_config(INTERFACE, MANUAL_ENABLED_KEY, &serde_json::Value::Bool(value))
    }
}
