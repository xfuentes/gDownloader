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

//! gDownloader's own local settings, for preferences JDownloader's config API
//! has no equivalent key for (e.g. the package tree's expand/collapse state,
//! which JD stores as a `Property` on its own `FilePackage`/`CrawledPackage`
//! objects — not reachable through the deprecated RemoteAPI). Stored as a
//! JSON file, separate from JDownloader's own config.
//!
//! Package UUIDs are stable across JDownloader restarts (they're persisted
//! as part of its own download-list/linkcollector save files), so keying
//! per-package state by uuid here survives gDownloader restarts too, as
//! long as the same JDownloader download list is loaded.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("gdownloader").join("config.json"))
}

fn load() -> Value {
    let Some(path) = config_path() else {
        return Value::Object(Default::default());
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Value::Object(Default::default());
    };
    serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Default::default()))
}

fn save(value: &Value) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(&path, text);
    }
}

/// Reads a `{key: value}` boolean map stored under `section` (e.g.
/// `"downloads_columns"` or `"linkgrabber"`).
pub fn get_bool_map(section: &str) -> HashMap<String, bool> {
    load()
        .get(section)
        .and_then(Value::as_object)
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_bool()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// Persists a single boolean value under `section`/`key`.
pub fn set_bool_entry(section: &str, key: &str, value: bool) {
    let mut root = load();
    let obj = root
        .as_object_mut()
        .expect("config root is always a JSON object");
    let section_obj = obj
        .entry(section.to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(section_obj) = section_obj.as_object_mut() {
        section_obj.insert(key.to_string(), Value::Bool(value));
    }
    save(&root);
}

/// Reads a `{uuid: expanded}` map stored under `section` (e.g. `"downloads"`
/// or `"linkgrabber"`), for restoring package tree expand/collapse state.
pub fn get_expanded_map(section: &str) -> HashMap<i64, bool> {
    get_bool_map(section)
        .into_iter()
        .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
        .collect()
}

/// Persists a single package's expand/collapse state under `section`.
pub fn set_expanded(section: &str, uuid: i64, expanded: bool) {
    set_bool_entry(section, &uuid.to_string(), expanded);
}
