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

/// Every option JDownloader's own "Add Links" dialog lets the user set,
/// passed to [`JdApi::add_links_with_options`]. Defaults match what
/// [`JdApi::add_links`] used to hardcode.
pub struct AddLinksOptions {
    pub links: String,
    pub autostart: bool,
    pub package_name: String,
    pub extract_password: String,
    pub download_password: String,
    pub destination_folder: String,
    pub comment: String,
    pub auto_extract: bool,
    pub deep_decrypt: bool,
    pub overwrite_packagizer_rules: bool,
    pub priority: String,
}

impl Default for AddLinksOptions {
    fn default() -> Self {
        Self {
            links: String::new(),
            autostart: false,
            package_name: String::new(),
            extract_password: String::new(),
            download_password: String::new(),
            destination_folder: String::new(),
            comment: String::new(),
            auto_extract: true,
            deep_decrypt: false,
            overwrite_packagizer_rules: false,
            priority: "DEFAULT".to_string(),
        }
    }
}

impl JdApi {
    /// Queries collected packages via `/linkgrabberv2/queryPackages`, including
    /// aggregate fields (size/etc.) for the package-level rows of the link
    /// grabber tree.
    pub fn query_linkcollector_packages(&self) -> Result<Vec<Value>> {
        let params = serde_json::json!({
            "startAt": 0,
            "maxResults": -1,
            "packageUUIDs": [],
            "childCount": true,
            "hosts": false,
            "bytesTotal": true,
            "comment": true,
            "enabled": true,
            "eta": false,
            "priority": false,
            "finished": true,
            "running": false,
            "speed": false,
            "status": false,
            "saveTo": true
        });
        match self.get_v2("linkgrabberv2/queryPackages", "queryParams", params)? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }

    /// Queries collected links via `/linkgrabberv2/queryLinks`.
    pub fn query_linkcollector_links(&self) -> Result<Vec<Value>> {
        let params = serde_json::json!({
            "startAt": 0,
            "maxResults": -1,
            "packageUUIDs": [],
            "linkUUIDs": [],
            "jobUUIDs": [],
            "host": true,
            "bytesTotal": true,
            "status": false,
            "advancedStatus": true,
            "availability": true,
            "enabled": true,
            "priority": true,
            "url": true,
            "uuid": true,
            "comment": true,
            "variantID": true,
            "variantName": true,
            "variantIcon": true,
            "addedDate": false
        });
        match self.get_v2("linkgrabberv2/queryLinks", "queryParams", params)? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }

    /// Lists the available quality/format variants for a link (e.g. video
    /// resolutions) via `/linkgrabberv2/getVariants`. Each entry has `id` and
    /// `name` fields.
    pub fn get_link_variants(&self, link_id: i64) -> Result<Vec<Value>> {
        match self.call("linkgrabberv2/getVariants", &[&link_id.to_string()])? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }

    /// Selects a link's active variant (by the id returned from
    /// [`get_link_variants`]) via `/linkgrabberv2/setVariant`.
    pub fn set_link_variant(&self, link_id: i64, variant_id: &str) -> Result<()> {
        let variant_id_json = serde_json::to_string(variant_id)?;
        self.call(
            "linkgrabberv2/setVariant",
            &[&link_id.to_string(), &variant_id_json],
        )?;
        Ok(())
    }

    /// Queries the link collector via `/linkgrabberv2/queryLinks` and
    /// `/linkgrabberv2/queryPackages`.
    ///
    /// Returns `(packages, links)`: `packages` are returned as-is (used for
    /// the package-level rows in the tree); `links` are merged with their
    /// package's `saveTo`/`name`.
    pub fn query_collector(&self) -> Result<(Vec<Value>, Vec<Value>)> {
        let links = self.query_linkcollector_links()?;
        let packages = self.query_linkcollector_packages()?;

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
            let save_to = package_uuid
                .and_then(|u| save_to_by_package.get(&u))
                .cloned()
                .unwrap_or_default();
            let package_name = package_uuid
                .and_then(|u| name_by_package.get(&u))
                .cloned()
                .unwrap_or_default();
            let mut link = link.clone();
            link["saveTo"] = Value::String(save_to);
            link["packageName"] = Value::String(package_name);
            result.push(link);
        }
        Ok((packages, result))
    }

    /// Adds links to the link collector via `/linkgrabberv2/addLinks`.
    ///
    /// Returns the crawl job's id (with `assignJobID` set, JDownloader tags
    /// every link that survives crawling/dupe-checking with it), usable
    /// with [`query_linkcollector_links_for_job`] to find out how many (and
    /// which) links this call actually added once crawling settles.
    pub fn add_links(&self, links: &str) -> Result<i64> {
        self.add_links_with_options(&AddLinksOptions {
            links: links.to_string(),
            ..Default::default()
        })
    }

    /// Same as [`add_links`], but exposes every option JDownloader's own
    /// "Add Links" dialog lets the user set.
    pub fn add_links_with_options(&self, opts: &AddLinksOptions) -> Result<i64> {
        let params = serde_json::json!({
            "autostart": opts.autostart,
            "links": opts.links,
            "packageName": opts.package_name,
            "extractPassword": opts.extract_password,
            "downloadPassword": opts.download_password,
            "destinationFolder": opts.destination_folder,
            "sourceUrl": "",
            "dataURLs": [],
            "comment": opts.comment,
            "autoExtract": opts.auto_extract,
            "deepDecrypt": opts.deep_decrypt,
            "overwritePackagizerRules": opts.overwrite_packagizer_rules,
            "assignJobID": true,
            "priority": opts.priority
        });
        let value = self.get_v2("linkgrabberv2/addLinks", "query", params)?;
        Ok(value.get("id").and_then(Value::as_i64).unwrap_or(0))
    }

    /// True while JDownloader is still crawling/checking previously-submitted
    /// links, via `/linkgrabberv2/isCollecting`.
    pub fn is_collecting(&self) -> Result<bool> {
        Ok(self
            .call("linkgrabberv2/isCollecting", &[])?
            .as_bool()
            .unwrap_or(false))
    }

    /// Queries the links belonging to a specific crawl job (from
    /// [`add_links`]'s returned id) via `/linkgrabberv2/queryLinks`, filtered
    /// by `jobUUIDs`. JDownloader's dupe manager drops already-collected
    /// links before they're ever persisted, so once crawling for the job has
    /// settled (see [`is_collecting`]), this reflects exactly the links that
    /// call actually added — not the ones that were already there.
    pub fn query_linkcollector_links_for_job(&self, job_id: i64) -> Result<Vec<Value>> {
        let params = serde_json::json!({
            "startAt": 0,
            "maxResults": -1,
            "packageUUIDs": [],
            "linkUUIDs": [],
            "jobUUIDs": [job_id],
            "uuid": true
        });
        match self.get_v2("linkgrabberv2/queryLinks", "queryParams", params)? {
            Value::Array(arr) => Ok(arr),
            _ => Ok(vec![]),
        }
    }

    /// Reorders link-collector packages via `/linkgrabberv2/movePackages`:
    /// moves `package_ids` to just after `after_dest_package_id`, or to the
    /// very top of the list when `after_dest_package_id <= 0`.
    pub fn move_linkgrabber_packages(&self, package_ids: &[i64], after_dest_package_id: i64) -> Result<()> {
        let package_ids_json = serde_json::to_string(package_ids)?;
        self.call(
            "linkgrabberv2/movePackages",
            &[&package_ids_json, &after_dest_package_id.to_string()],
        )?;
        Ok(())
    }

    /// Removes links from the link collector via `/linkgrabberv2/removeLinks`.
    pub fn remove_linkgrabber_links(&self, link_ids: &[i64]) -> Result<()> {
        let link_ids_json = serde_json::to_string(link_ids)?;
        self.call("linkgrabberv2/removeLinks", &[&link_ids_json, "[]"])?;
        Ok(())
    }

    /// Renames a single link via `/linkgrabberv2/renameLink`.
    pub fn rename_linkgrabber_link(&self, link_id: i64, new_name: &str) -> Result<()> {
        let new_name_json = serde_json::to_string(new_name)?;
        self.call("linkgrabberv2/renameLink", &[&link_id.to_string(), &new_name_json])?;
        Ok(())
    }

    /// Renames a package via `/linkgrabberv2/renamePackage`.
    pub fn rename_linkgrabber_package(&self, package_id: i64, new_name: &str) -> Result<()> {
        let new_name_json = serde_json::to_string(new_name)?;
        self.call(
            "linkgrabberv2/renamePackage",
            &[&package_id.to_string(), &new_name_json],
        )?;
        Ok(())
    }

    /// Sets the comment on the given links/packages via `/linkgrabberv2/setComment`.
    pub fn set_linkgrabber_comment(
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
            "linkgrabberv2/setComment",
            &[
                &link_ids_json,
                &package_ids_json,
                &all_package_links.to_string(),
                &comment_json,
            ],
        )?;
        Ok(())
    }

    /// Sets the priority of the given links/packages via `/linkgrabberv2/setPriority`.
    ///
    /// `priority` is one of JDownloader's `Priority` enum names: `"HIGHEST"`,
    /// `"HIGHER"`, `"HIGH"`, `"DEFAULT"`, `"LOW"`, `"LOWER"`, `"LOWEST"`.
    pub fn set_linkgrabber_priority(
        &self,
        link_ids: &[i64],
        package_ids: &[i64],
        priority: &str,
    ) -> Result<()> {
        let priority_json = serde_json::to_string(priority)?;
        let link_ids_json = serde_json::to_string(link_ids)?;
        let package_ids_json = serde_json::to_string(package_ids)?;
        self.call(
            "linkgrabberv2/setPriority",
            &[&priority_json, &link_ids_json, &package_ids_json],
        )?;
        Ok(())
    }

    /// Sets the download directory of the given packages via
    /// `/linkgrabberv2/setDownloadDirectory`. This is package-scoped only.
    pub fn set_linkgrabber_directory(&self, directory: &str, package_ids: &[i64]) -> Result<()> {
        let directory_json = serde_json::to_string(directory)?;
        let package_ids_json = serde_json::to_string(package_ids)?;
        self.call(
            "linkgrabberv2/setDownloadDirectory",
            &[&directory_json, &package_ids_json],
        )?;
        Ok(())
    }

    /// Starts the selected link collector links via `/linkcollector/startDownloads`.
    pub fn start_linkgrabber_downloads(&self, link_ids: &[u64]) -> Result<bool> {
        let ids_json = serde_json::to_string(link_ids)?;
        self.call("linkcollector/startDownloads", &[&ids_json, "[]"])
            .map(|v| v.as_bool().unwrap_or(false))
    }

    /// Moves links to the download list via `/linkgrabberv2/moveToDownloadlist`,
    /// without starting them — unlike [`Self::start_linkgrabber_downloads`],
    /// which moves *and* starts.
    pub fn move_linkgrabber_to_downloadlist(&self, link_ids: &[i64]) -> Result<()> {
        let link_ids_json = serde_json::to_string(link_ids)?;
        self.call("linkgrabberv2/moveToDownloadlist", &[&link_ids_json, "[]"])?;
        Ok(())
    }
}
