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

use super::JdApi;

impl JdApi {
    pub fn is_ready(&self) -> bool {
        if let Ok(r) = self
            .client
            .get(format!("{}/", self.base_url))
            .timeout(Duration::from_millis(1000))
            .send()
        {
            if r.status().is_success() {
                return true;
            }
            if r.json::<serde_json::Value>().is_ok() {
                return true;
            }
        }
        false
    }

    /// Asks JDownloader to shut down via `/system/exitJD`.
    pub fn system_exit(&self) -> Result<()> {
        let url = format!("{}/system/exitJD", self.base_url);
        debug!("JDownloader API request: {}", url);
        let _ = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send();
        Ok(())
    }

    /// Triggers a reconnect via `/reconnect/doReconnect`.
    pub fn do_reconnect(&self) -> Result<()> {
        self.call("reconnect/doReconnect", &[])?;
        Ok(())
    }

    /// Triggers an update check via `/update/runUpdateCheck`.
    pub fn run_update_check(&self) -> Result<()> {
        self.call("update/runUpdateCheck", &[])?;
        Ok(())
    }

    /// True when an update has been found and is ready to install, via
    /// `/update/isUpdateAvailable`.
    pub fn is_update_available(&self) -> Result<bool> {
        Ok(self
            .call("update/isUpdateAvailable", &[])?
            .as_bool()
            .unwrap_or(false))
    }
}
