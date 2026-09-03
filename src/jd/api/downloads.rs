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

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;

use super::JdApi;

/// Snapshot of JDownloader's global download/toolbar state, from
/// `/toolbar/getStatus`. Backs the main toolbar's start/pause/stop
/// enabled-state and its clipboard/auto-reconnect/premium toggle states.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolbarStatus {
    pub running: bool,
    pub pause: bool,
    pub auto_reconnect: bool,
    pub clipboard_monitored: bool,
    pub use_premium_accounts: bool,
}

impl JdApi {
    /// Starts the global download queue via `/toolbar/startDownloads`.
    pub fn start_all_downloads(&self) -> Result<bool> {
        self.call("toolbar/startDownloads", &[])
            .map(|v| v.as_bool().unwrap_or(false))
    }

    /// Stops the global download queue via `/toolbar/stopDownloads`.
    pub fn stop_all_downloads(&self) -> Result<bool> {
        self.call("toolbar/stopDownloads", &[])
            .map(|v| v.as_bool().unwrap_or(false))
    }

    /// Toggles the pause state via `/toolbar/togglePauseDownloads`.
    pub fn toggle_pause_downloads(&self) -> Result<bool> {
        self.call("toolbar/togglePauseDownloads", &[])
            .map(|v| v.as_bool().unwrap_or(false))
    }

    /// Reads JDownloader's global toolbar/download state via
    /// `/toolbar/getStatus`.
    pub fn get_toolbar_status(&self) -> Result<ToolbarStatus> {
        let v = self.call("toolbar/getStatus", &[])?;
        let get_bool = |key: &str| v.get(key).and_then(Value::as_bool).unwrap_or(false);
        Ok(ToolbarStatus {
            running: get_bool("running"),
            pause: get_bool("pause"),
            auto_reconnect: get_bool("reconnect"),
            clipboard_monitored: get_bool("clipboard"),
            use_premium_accounts: get_bool("premium"),
        })
    }

    /// Reorders download packages via `/downloadsV2/movePackages`: moves
    /// `package_ids` to just after `after_dest_package_id`, or to the very
    /// top of the list when `after_dest_package_id <= 0`.
    pub fn move_download_packages(&self, package_ids: &[i64], after_dest_package_id: i64) -> Result<()> {
        let package_ids_json = serde_json::to_string(package_ids)?;
        self.call(
            "downloadsV2/movePackages",
            &[&package_ids_json, &after_dest_package_id.to_string()],
        )?;
        Ok(())
    }

    /// Queries downloads via `/downloadsV2/queryLinks` and `/downloadsV2/queryPackages`.
    ///
    /// Returns `(packages, links)`: `packages` are returned as-is (each with
    /// its own aggregate `bytesTotal`/`bytesLoaded`/`status`/etc., used for
    /// the package-level rows in the tree); `links` are merged with their
    /// package's `saveTo`/`name` to populate `downloadPath`/`packageName`.
    pub fn query_downloads(&self) -> Result<(Vec<Value>, Vec<Value>)> {
        let links = self.query_downloads_links()?;
        let packages = self.query_downloads_packages()?;

        let mut save_to_by_package: HashMap<u64, String> = HashMap::new();
        let mut name_by_package: HashMap<u64, String> = HashMap::new();
        for pkg in &packages {
            let uuid = pkg.get("uuid").and_then(Value::as_u64).unwrap_or(0);
            if let Some(save_to) = pkg.get("saveTo").and_then(Value::as_str) {
                save_to_by_package.insert(uuid, save_to.to_string());
            }
            if let Some(name) = pkg.get("name").and_then(Value::as_str) {
                name_by_package.insert(uuid, name.to_string());
            }
        }

        let mut result = Vec::new();
        for link in &links {
            let package_uuid = link.get("packageUUID").and_then(Value::as_u64);
            let download_path = package_uuid
                .and_then(|u| save_to_by_package.get(&u))
                .cloned()
                .unwrap_or_default();
            let package_name = package_uuid
                .and_then(|u| name_by_package.get(&u))
                .cloned()
                .unwrap_or_default();
            let mut link = link.clone();
            link["downloadPath"] = Value::String(download_path);
            link["packageName"] = Value::String(package_name);
            result.push(link);
        }
        Ok((packages, result))
    }

    /// Queries download packages via `/downloadsV2/queryPackages`, including
    /// aggregate fields (size/progress/status/etc.) for the package-level
    /// rows of the downloads tree.
    pub fn query_downloads_packages(&self) -> Result<Vec<Value>> {
        let params = serde_json::json!({
            "startAt": 0,
            "maxResults": -1,
            "packageUUIDs": [],
            "childCount": true,
            "hosts": false,
            "bytesTotal": true,
            "bytesLoaded": true,
            "comment": true,
            "enabled": true,
            "eta": true,
            "priority": false,
            "finished": true,
            "running": true,
            "speed": true,
            "status": true,
            "saveTo": true
        });
        match self.get_v2("downloadsV2/queryPackages", "queryParams", params)? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }

    /// Removes the given links via `/downloadsV2/cleanup`, optionally also
    /// recycling or permanently deleting their downloaded files.
    ///
    /// `mode` is one of JDownloader's `DeleteFileOptions` enum names:
    /// `"REMOVE_LINKS_ONLY"`, `"REMOVE_LINKS_AND_RECYCLE_FILES"`, or
    /// `"REMOVE_LINKS_AND_DELETE_FILES"`. Always passes `action=DELETE_ALL`
    /// with `selectionType=SELECTED` so the server deletes exactly the given
    /// `link_ids` instead of applying a smart filter (JDownloader's other
    /// `action` values like `DELETE_FINISHED` ignore the explicit id list).
    pub fn cleanup_downloads(&self, link_ids: &[i64], mode: &str) -> Result<()> {
        let link_ids_json = serde_json::to_string(link_ids)?;
        let action_json = serde_json::to_string("DELETE_ALL")?;
        let mode_json = serde_json::to_string(mode)?;
        let selection_type_json = serde_json::to_string("SELECTED")?;
        self.call(
            "downloadsV2/cleanup",
            &[
                &link_ids_json,
                "[]",
                &action_json,
                &mode_json,
                &selection_type_json,
            ],
        )?;
        Ok(())
    }

    /// Renames a single link via `/downloadsV2/renameLink`.
    pub fn rename_download_link(&self, link_id: i64, new_name: &str) -> Result<()> {
        let new_name_json = serde_json::to_string(new_name)?;
        self.call("downloadsV2/renameLink", &[&link_id.to_string(), &new_name_json])?;
        Ok(())
    }

    /// Renames a package via `/downloadsV2/renamePackage`.
    pub fn rename_download_package(&self, package_id: i64, new_name: &str) -> Result<()> {
        let new_name_json = serde_json::to_string(new_name)?;
        self.call(
            "downloadsV2/renamePackage",
            &[&package_id.to_string(), &new_name_json],
        )?;
        Ok(())
    }

    /// Sets the comment on the given links/packages via `/downloadsV2/setComment`.
    pub fn set_download_comment(
        &self,
        link_ids: &[i64],
        package_ids: &[i64],
        all_package_links: bool,
        comment: &str,
    ) -> Result<()> {
        let link_ids_json = serde_json::to_string(link_ids)?;
        let package_ids_json = serde_json::to_string(package_ids)?;
        let comment_json = serde_json::to_string(comment)?;
        self.call(
            "downloadsV2/setComment",
            &[
                &link_ids_json,
                &package_ids_json,
                &all_package_links.to_string(),
                &comment_json,
            ],
        )?;
        Ok(())
    }

    /// Sets the priority of the given links/packages via `/downloadsV2/setPriority`.
    ///
    /// `priority` is one of JDownloader's `Priority` enum names: `"HIGHEST"`,
    /// `"HIGHER"`, `"HIGH"`, `"DEFAULT"`, `"LOW"`, `"LOWER"`, `"LOWEST"`.
    pub fn set_download_priority(
        &self,
        link_ids: &[i64],
        package_ids: &[i64],
        priority: &str,
    ) -> Result<()> {
        let priority_json = serde_json::to_string(priority)?;
        let link_ids_json = serde_json::to_string(link_ids)?;
        let package_ids_json = serde_json::to_string(package_ids)?;
        self.call(
            "downloadsV2/setPriority",
            &[&priority_json, &link_ids_json, &package_ids_json],
        )?;
        Ok(())
    }

    /// Sets the download directory of the given packages via
    /// `/downloadsV2/setDownloadDirectory`. This is package-scoped only.
    pub fn set_download_directory(&self, directory: &str, package_ids: &[i64]) -> Result<()> {
        let directory_json = serde_json::to_string(directory)?;
        let package_ids_json = serde_json::to_string(package_ids)?;
        self.call(
            "downloadsV2/setDownloadDirectory",
            &[&directory_json, &package_ids_json],
        )?;
        Ok(())
    }

    /// Queries download links via `/downloadsV2/queryLinks`.
    pub fn query_downloads_links(&self) -> Result<Vec<Value>> {
        let params = serde_json::json!({
            "startAt": 0,
            "maxResults": -1,
            "packageUUIDs": [],
            "linkUUIDs": [],
            "jobUUIDs": [],
            "host": true,
            "name": true,
            "comment": true,
            "bytesTotal": true,
            "bytesLoaded": true,
            "status": true,
            "advancedStatus": true,
            "finished": true,
            "running": true,
            "enabled": true,
            "speed": true,
            "url": true,
            "uuid": true,
            "priority": true,
            "password": false,
            "eta": true,
            "skipped": true,
            "jobUUID": false,
            "addedDate": false,
            "finishedDate": false,
            "extractionStatus": false
        });
        match self.get_v2("downloadsV2/queryLinks", "queryParams", params)? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }
}
