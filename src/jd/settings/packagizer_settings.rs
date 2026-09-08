use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jd::JdApi;

const INTERFACE: &str = "org.jdownloader.controlling.packagizer.PackagizerSettings";
const RULE_LIST_KEY: &str = "RuleList";
const PACKAGIZER_ENABLED_KEY: &str = "PackagizerEnabled";

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Mirrors `org.jdownloader.controlling.filter.RegexFilter.MatchType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegexMatchType {
    #[default]
    #[serde(rename = "CONTAINS")]
    Contains,
    #[serde(rename = "EQUALS")]
    Equals,
    #[serde(rename = "CONTAINS_NOT")]
    ContainsNot,
    #[serde(rename = "EQUALS_NOT")]
    EqualsNot,
}

/// Mirrors `org.jdownloader.controlling.filter.FilesizeFilter.SizeMatchType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeMatchType {
    #[default]
    #[serde(rename = "BETWEEN")]
    Between,
    #[serde(rename = "NOT_BETWEEN")]
    NotBetween,
}

/// Mirrors `org.jdownloader.controlling.filter.FiletypeFilter.TypeMatchType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeMatchType {
    #[default]
    #[serde(rename = "IS")]
    Is,
    #[serde(rename = "IS_NOT")]
    IsNot,
}

/// Mirrors `org.jdownloader.controlling.Priority`. Variant order matches
/// `Priority.values()` (used to populate the priority chooser).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    #[serde(rename = "HIGHEST")]
    Highest,
    #[serde(rename = "HIGHER")]
    Higher,
    #[serde(rename = "HIGH")]
    High,
    #[serde(rename = "DEFAULT")]
    Default,
    #[serde(rename = "LOW")]
    Low,
    #[serde(rename = "LOWER")]
    Lower,
    #[serde(rename = "LOWEST")]
    Lowest,
}

impl Priority {
    pub const ALL: [Priority; 7] = [
        Priority::Highest,
        Priority::Higher,
        Priority::High,
        Priority::Default,
        Priority::Low,
        Priority::Lower,
        Priority::Lowest,
    ];
}

/// Mirrors `org.jdownloader.controlling.filter.RegexFilter`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RegexFilter {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "matchType")]
    pub match_type: RegexMatchType,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default, rename = "useRegex")]
    pub use_regex: bool,
}

/// Mirrors `org.jdownloader.controlling.filter.FilesizeFilter`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FilesizeFilter {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "matchType")]
    pub match_type: SizeMatchType,
    #[serde(default)]
    pub from: i64,
    #[serde(default)]
    pub to: i64,
}

/// Mirrors `org.jdownloader.controlling.filter.FiletypeFilter`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FiletypeFilter {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "matchType")]
    pub match_type: TypeMatchType,
    #[serde(default, rename = "useRegex")]
    pub use_regex: bool,
    #[serde(default, rename = "hashEnabled")]
    pub hash_enabled: bool,
    #[serde(default, rename = "audioFilesEnabled")]
    pub audio_files_enabled: bool,
    #[serde(default, rename = "videoFilesEnabled")]
    pub video_files_enabled: bool,
    #[serde(default, rename = "archivesEnabled")]
    pub archives_enabled: bool,
    #[serde(default, rename = "imagesEnabled")]
    pub images_enabled: bool,
    #[serde(default, rename = "docFilesEnabled")]
    pub doc_files_enabled: bool,
    #[serde(default, rename = "subFilesEnabled")]
    pub sub_files_enabled: bool,
    #[serde(default, rename = "exeFilesEnabled")]
    pub exe_files_enabled: bool,
    #[serde(default)]
    pub customs: Option<String>,
}

/// Mirrors `org.jdownloader.controlling.filter.BooleanFilter`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BooleanFilter {
    #[serde(default)]
    pub enabled: bool,
}

/// Mirrors `org.jdownloader.controlling.packagizer.PackagizerRule` (which
/// extends `org.jdownloader.controlling.filter.FilterRule`).
///
/// Condition/action fields with an editor in gDownloader are fully typed so
/// they round-trip losslessly with real JDownloader. Fields gDownloader does
/// not expose an editor for (e.g. `originFilter`, `pluginStatusFilter`) are
/// kept as raw `Value` so rules created in JDownloader itself are never
/// silently stripped when re-saved from here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PackagizerRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "staticRule")]
    pub static_rule: bool,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default = "now_millis")]
    pub created: i64,
    #[serde(default, rename = "iconKey")]
    pub icon_key: Option<String>,
    #[serde(default, rename = "testUrl")]
    pub test_url: Option<String>,

    // Conditions
    #[serde(default, rename = "matchAlwaysFilter")]
    pub match_always_filter: BooleanFilter,
    #[serde(default, rename = "filenameFilter")]
    pub filename_filter: RegexFilter,
    #[serde(default, rename = "packagenameFilter")]
    pub packagename_filter: RegexFilter,
    #[serde(default, rename = "commentFilter")]
    pub comment_filter: RegexFilter,
    #[serde(default, rename = "hosterURLFilter")]
    pub hoster_url_filter: RegexFilter,
    #[serde(default, rename = "sourceURLFilter")]
    pub source_url_filter: RegexFilter,
    #[serde(default, rename = "filesizeFilter")]
    pub filesize_filter: FilesizeFilter,
    #[serde(default, rename = "filetypeFilter")]
    pub filetype_filter: FiletypeFilter,

    // Conditions without an editor: preserved verbatim.
    #[serde(default, rename = "onlineStatusFilter", skip_serializing_if = "Option::is_none")]
    pub online_status_filter: Option<Value>,
    #[serde(default, rename = "originFilter", skip_serializing_if = "Option::is_none")]
    pub origin_filter: Option<Value>,
    #[serde(default, rename = "linkEnabledFilter", skip_serializing_if = "Option::is_none")]
    pub link_enabled_filter: Option<Value>,
    #[serde(
        default,
        rename = "downloadListDupeFilter",
        skip_serializing_if = "Option::is_none"
    )]
    pub download_list_dupe_filter: Option<Value>,
    #[serde(default, rename = "pluginStatusFilter", skip_serializing_if = "Option::is_none")]
    pub plugin_status_filter: Option<Value>,
    #[serde(default, rename = "conditionFilter", skip_serializing_if = "Option::is_none")]
    pub condition_filter: Option<Value>,

    // "...then" actions
    #[serde(default, rename = "downloadDestination")]
    pub download_destination: Option<String>,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default, rename = "packageName")]
    pub package_name: Option<String>,
    #[serde(default, rename = "packageKey")]
    pub package_key: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    /// Number of chunks, or `-1`/`0` when the action is disabled (matches the
    /// Java dialog, which writes `-1` when the "chunks" checkbox is unset).
    #[serde(default)]
    pub chunks: i32,
    #[serde(default, rename = "autoExtractionEnabled")]
    pub auto_extraction_enabled: Option<bool>,
    #[serde(default, rename = "autoAddEnabled")]
    pub auto_add_enabled: Option<bool>,
    #[serde(default, rename = "autoStartEnabled")]
    pub auto_start_enabled: Option<bool>,
    #[serde(default, rename = "autoForcedStartEnabled")]
    pub auto_forced_start_enabled: Option<bool>,
    #[serde(default, rename = "linkEnabled")]
    pub link_enabled: Option<bool>,

    // "...and do" actions
    #[serde(default)]
    pub moveto: Option<String>,
    #[serde(default)]
    pub rename: Option<String>,

    #[serde(default)]
    pub order: i32,
    #[serde(default, rename = "stopAfterThisRule")]
    pub stop_after_this_rule: bool,
}

impl PackagizerRule {
    /// A fresh, enabled rule with no conditions/actions set yet, matching
    /// what JDownloader's "New" action produces.
    pub fn new(name: &str) -> Self {
        Self {
            name: Some(name.to_string()),
            enabled: true,
            created: now_millis(),
            chunks: -1,
            ..Default::default()
        }
    }
}

/// Accessor for JDownloader's `PackagizerSettings` (the "Package Manager" /
/// "Gestion des paquets" rule engine).
#[derive(Clone)]
pub struct PackagizerSettings {
    api: Arc<JdApi>,
}

impl PackagizerSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn is_ready(&self) -> bool {
        self.api.is_ready()
    }

    pub fn get_rule_list(&self) -> anyhow::Result<Vec<PackagizerRule>> {
        let value = self.api.get_config(INTERFACE, RULE_LIST_KEY)?;
        if value.is_null() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_value(value)?)
    }

    pub fn set_rule_list(&self, rules: &[PackagizerRule]) -> anyhow::Result<bool> {
        let value = serde_json::to_value(rules)?;
        self.api.set_config(INTERFACE, RULE_LIST_KEY, &value)
    }

    pub fn get_packagizer_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, PACKAGIZER_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn set_packagizer_enabled(&self, value: bool) -> anyhow::Result<bool> {
        let v = serde_json::to_value(value)?;
        self.api.set_config(INTERFACE, PACKAGIZER_ENABLED_KEY, &v)
    }
}
