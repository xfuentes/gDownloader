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

use anyhow::Result;
use serde_json::Value;

use super::JdApi;

impl JdApi {
    /// Starts extraction now for the given links/packages via
    /// `/extraction/startExtractionNow`, bypassing the "extract after
    /// download finishes" trigger — mirrors the context menu's
    /// "Extract Now" (`ExtractArchiveNowAction`).
    pub fn start_extraction_now(&self, link_ids: &[i64], package_ids: &[i64]) -> Result<()> {
        let link_ids_json = serde_json::to_string(link_ids)?;
        let package_ids_json = serde_json::to_string(package_ids)?;
        self.call(
            "extraction/startExtractionNow",
            &[&link_ids_json, &package_ids_json],
        )?;
        Ok(())
    }

    /// Cancels a running/queued extraction via `/extraction/cancelExtraction`
    /// — mirrors the context menu's "Abort extraction"
    /// (`AbortExtractionAction`). `controller_id` comes from
    /// [`Self::get_archive_info`]'s `controllerId`.
    pub fn cancel_extraction(&self, controller_id: i64) -> Result<bool> {
        Ok(self
            .call("extraction/cancelExtraction", &[&controller_id.to_string()])?
            .as_bool()
            .unwrap_or(false))
    }

    /// Resolves archives for the given links/packages via
    /// `/extraction/getArchiveInfo`. Each returned object mirrors
    /// `ArchiveStatusStorable`: `archiveId`, `archiveName`, `controllerId`
    /// (-1 if no controller is active), `controllerStatus`
    /// (`"RUNNING"`/`"QUEUED"`/`"NA"`), `type`, and `states` (a map of
    /// part-filename to `"COMPLETE"`/`"INCOMPLETE"`/`"MISSING"`). An empty
    /// selection or one with no archives in it returns an empty list.
    pub fn get_archive_info(&self, link_ids: &[i64], package_ids: &[i64]) -> Result<Vec<Value>> {
        let link_ids_json = serde_json::to_string(link_ids)?;
        let package_ids_json = serde_json::to_string(package_ids)?;
        match self.call(
            "extraction/getArchiveInfo",
            &[&link_ids_json, &package_ids_json],
        )? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }

    /// Reads per-archive extraction settings via
    /// `/extraction/getArchiveSettings`. Each returned object mirrors
    /// `ArchiveSettingsAPIStorable` (`archiveId`, `autoExtract`,
    /// `extractPath`, `finalPassword`, `passwords`,
    /// `removeDownloadLinksAfterExtraction`, `removeFilesAfterExtraction`) —
    /// any field left `null` means "use the global default".
    pub fn get_archive_settings(&self, archive_ids: &[&str]) -> Result<Vec<Value>> {
        let archive_ids_json = serde_json::to_string(archive_ids)?;
        match self.call("extraction/getArchiveSettings", &[&archive_ids_json])? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }

    /// Writes per-archive extraction settings via
    /// `/extraction/setArchiveSettings` — see [`Self::get_archive_settings`]
    /// for the field shape. Only the fields present in `settings` are
    /// changed; JDownloader keeps the rest as they were.
    pub fn set_archive_settings(&self, archive_id: &str, settings: Value) -> Result<bool> {
        let settings_json = settings.to_string();
        Ok(self
            .call("extraction/setArchiveSettings", &[archive_id, &settings_json])?
            .as_bool()
            .unwrap_or(false))
    }

    /// Adds a password to JDownloader's suggested-password pool via
    /// `/extraction/addArchivePassword` — mirrors the context menu's "Set
    /// Archive Password" partially; callers also want
    /// [`Self::set_archive_settings`] with `finalPassword` to apply it to a
    /// specific archive right away, like the real menu action does.
    pub fn add_archive_password(&self, password: &str) -> Result<()> {
        self.call("extraction/addArchivePassword", &[password])?;
        Ok(())
    }
}
