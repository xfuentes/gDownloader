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

use std::time::Duration;

use anyhow::Result;
use log::debug;
use serde_json::Value;

use super::{unwrap_value, JdApi};

impl JdApi {
    /// Reads a configuration value via `/config/get`.
    ///
    /// The call is of the form `/config/get?{interface}&{storage}&{key}`.
    /// `storage` is `"null"` to use the default storage — true for
    /// JDownloader's own top-level `@ConfigInterface`s, but *not* for a
    /// config interface owned by an extension: `AbstractExtension.buildStore()`
    /// stores those under `cfg/<extension classname>` rather than
    /// `cfg/<config interface name>`, which JDownloader's `StorageHandler`
    /// resolves into a non-null storage id (a relative path string) — use
    /// [`Self::get_config_with_storage`] for those, or the request silently
    /// finds no key handler and returns an empty value.
    pub fn get_config(&self, interface: &str, key: &str) -> Result<Value> {
        self.get_config_with_storage(interface, "null", key)
    }

    /// Like [`Self::get_config`], but for a config interface that isn't
    /// registered under the default (`"null"`) storage — see its docs.
    pub fn get_config_with_storage(&self, interface: &str, storage: &str, key: &str) -> Result<Value> {
        let url = format!("{}/config/get?{}&{}&{}", self.base_url, interface, storage, key);
        debug!("JDownloader API request: {}", url);
        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()?;
        let status = response.status();
        let text = response.text()?;
        debug!("JDownloader API response ({} {}): {}", status, key, text);
        if !status.is_success() {
            anyhow::bail!("JDownloader API returned {}: {}", status, text);
        }
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Invalid JSON from JDownloader ({}): {}", e, text))?;
        Ok(unwrap_value(value))
    }

    /// Writes a configuration value via `/config/set`.
    ///
    /// The call is of the form `/config/set?{interface}&{storage}&{key}&{value}`.
    /// `value` is JSON-encoded and passed as the last positional parameter.
    /// See [`Self::get_config`] on why `storage` is hardcoded to `"null"`
    /// here — use [`Self::set_config_with_storage`] for an extension-owned
    /// config interface.
    pub fn set_config(&self, interface: &str, key: &str, value: &Value) -> Result<bool> {
        self.set_config_with_storage(interface, "null", key, value)
    }

    /// Like [`Self::set_config`], but for a config interface that isn't
    /// registered under the default (`"null"`) storage — see
    /// [`Self::get_config`]'s docs.
    pub fn set_config_with_storage(
        &self,
        interface: &str,
        storage: &str,
        key: &str,
        value: &Value,
    ) -> Result<bool> {
        let url = format!("{}/config/set", self.base_url);
        let value_json = value.to_string();
        debug!(
            "JDownloader API set request: {}?{}&{}&{}&{}",
            url, interface, storage, key, value_json
        );
        let response = self
            .client
            .get(&url)
            .query(&[
                ("interface", interface),
                ("storage", storage),
                ("key", key),
                ("value", &value_json),
            ])
            .timeout(Duration::from_secs(5))
            .send()?;
        let status = response.status();
        let text = response.text()?;
        debug!(
            "JDownloader API set response ({} {}): {}",
            status, key, text
        );
        if !status.is_success() {
            anyhow::bail!("JDownloader API set returned {}: {}", status, text);
        }
        let value: Value = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("Invalid JSON from JDownloader set ({}): {}", e, text)
        })?;
        match unwrap_value(value) {
            Value::Bool(b) => Ok(b),
            _ => Ok(false),
        }
    }

    /// Triggers a favicon download via `/contentV2/getFavIcon`.
    ///
    /// The deprecated API does not always return the icon synchronously; the
    /// actual PNG is written to the theme's `images/fav/` directory once the
    /// background download completes. This method just asks JDownloader to start.
    pub fn trigger_favicon(&self, host: &str) -> Result<()> {
        let url = format!("{}/contentV2/getFavIcon", self.base_url);
        debug!("JDownloader favicon trigger: {} for {}", url, host);
        let _ = self
            .client
            .get(&url)
            .query(&[("hostername", host)])
            .timeout(Duration::from_secs(10))
            .send()?;
        Ok(())
    }

    /// Fetches an icon's PNG bytes via `/contentV2/getIcon`.
    ///
    /// Unlike `/contentV2/getFavIcon`, this endpoint returns the image
    /// directly in the response body rather than writing it to a themed
    /// directory, since `key` may be an opaque, server-side composited
    /// descriptor (e.g. a variant's `"kc.<hash>"` icon key) with no file on
    /// disk to begin with.
    pub fn fetch_icon(&self, key: &str, size: u32) -> Result<Vec<u8>> {
        let url = format!("{}/contentV2/getIcon", self.base_url);
        let size_str = size.to_string();
        debug!("JDownloader icon fetch: {} key={} size={}", url, key, size_str);
        let response = self
            .client
            .get(&url)
            .query(&[("key", key), ("size", &size_str)])
            .timeout(Duration::from_secs(10))
            .send()?
            .error_for_status()?;
        Ok(response.bytes()?.to_vec())
    }
}
