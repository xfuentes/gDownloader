use std::sync::Arc;

use crate::jd::JdApi;
use serde_json::Value;

const INTERFACE: &str = "org.jdownloader.settings.GeneralSettings";
const DEFAULT_DOWNLOAD_FOLDER_KEY: &str = "DefaultDownloadFolder";
const MAX_SIMULTANE_DOWNLOADS_KEY: &str = "MaxSimultaneDownloads";
const MAX_DOWNLOADS_PER_HOST_ENABLED_KEY: &str = "MaxDownloadsPerHostEnabled";
const MAX_SIMULTANE_DOWNLOADS_PER_HOST_KEY: &str = "MaxSimultaneDownloadsPerHost";
const MAX_CHUNKS_PER_FILE_KEY: &str = "MaxChunksPerFile";
const CLEANUP_AFTER_DOWNLOAD_ACTION_KEY: &str = "CleanupAfterDownloadAction";
const IF_FILE_EXISTS_ACTION_KEY: &str = "IfFileExistsAction";
const AUTO_START_DOWNLOAD_OPTION_KEY: &str = "AutoStartDownloadOption";
const SHOW_COUNTDOWN_ON_AUTO_START_DOWNLOADS_KEY: &str = "ShowCountdownonAutoStartDownloads";
const AUTO_START_COUNTDOWN_SECONDS_KEY: &str = "AutoStartCountdownSeconds";
const HASH_CHECK_ENABLED_KEY: &str = "HashCheckEnabled";
const HASH_RETRY_ENABLED_KEY: &str = "HashRetryEnabled";
const AUTO_OPEN_CONTAINER_AFTER_DOWNLOAD_KEY: &str = "AutoOpenContainerAfterDownload";
const USE_AVAILABLE_ACCOUNTS_KEY: &str = "UseAvailableAccounts";
const DOWNLOAD_SPEED_LIMIT_ENABLED_KEY: &str = "DownloadSpeedLimitEnabled";

#[derive(Clone)]
/// Accessor for JDownloader's GeneralSettings.
pub struct GeneralSettings {
    api: Arc<JdApi>,
}

impl GeneralSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn is_ready(&self) -> bool {
        self.api.is_ready()
    }

    fn get(&self, key: &str) -> anyhow::Result<serde_json::Value> {
        self.api.get_config(INTERFACE, key)
    }

    fn set<T: serde::Serialize>(&self, key: &str, value: T) -> anyhow::Result<bool> {
        let value = serde_json::to_value(value)?;
        self.api.set_config(INTERFACE, key, &value)
    }

    pub fn get_default_download_folder(&self) -> anyhow::Result<String> {
        match self.get(DEFAULT_DOWNLOAD_FOLDER_KEY)? {
            Value::String(s) => Ok(s),
            v => Ok(v.as_str().unwrap_or("").to_string()),
        }
    }

    pub fn set_default_download_folder(&self, value: &str) -> anyhow::Result<bool> {
        self.set(DEFAULT_DOWNLOAD_FOLDER_KEY, value)
    }

    pub fn get_max_simultane_downloads(&self) -> anyhow::Result<i32> {
        Ok(self.get(MAX_SIMULTANE_DOWNLOADS_KEY)?.as_i64().unwrap_or(3) as i32)
    }

    pub fn set_max_simultane_downloads(&self, value: i32) -> anyhow::Result<bool> {
        self.set(MAX_SIMULTANE_DOWNLOADS_KEY, value)
    }

    pub fn get_max_downloads_per_host_enabled(&self) -> anyhow::Result<bool> {
        Ok(self.get(MAX_DOWNLOADS_PER_HOST_ENABLED_KEY)?.as_bool().unwrap_or(false))
    }

    pub fn set_max_downloads_per_host_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.set(MAX_DOWNLOADS_PER_HOST_ENABLED_KEY, value)
    }

    pub fn get_max_simultane_downloads_per_host(&self) -> anyhow::Result<i32> {
        Ok(self.get(MAX_SIMULTANE_DOWNLOADS_PER_HOST_KEY)?.as_i64().unwrap_or(1) as i32)
    }

    pub fn set_max_simultane_downloads_per_host(&self, value: i32) -> anyhow::Result<bool> {
        self.set(MAX_SIMULTANE_DOWNLOADS_PER_HOST_KEY, value)
    }

    pub fn get_max_chunks_per_file(&self) -> anyhow::Result<i32> {
        Ok(self.get(MAX_CHUNKS_PER_FILE_KEY)?.as_i64().unwrap_or(1) as i32)
    }

    pub fn set_max_chunks_per_file(&self, value: i32) -> anyhow::Result<bool> {
        self.set(MAX_CHUNKS_PER_FILE_KEY, value)
    }

    pub fn get_cleanup_after_download_action(&self) -> anyhow::Result<String> {
        match self.get(CLEANUP_AFTER_DOWNLOAD_ACTION_KEY)? {
            Value::String(s) => Ok(s),
            v => Ok(v.as_str().unwrap_or("NEVER").to_string()),
        }
    }

    pub fn set_cleanup_after_download_action(&self, value: &str) -> anyhow::Result<bool> {
        self.set(CLEANUP_AFTER_DOWNLOAD_ACTION_KEY, value)
    }

    pub fn get_if_file_exists_action(&self) -> anyhow::Result<String> {
        match self.get(IF_FILE_EXISTS_ACTION_KEY)? {
            Value::String(s) => Ok(s),
            v => Ok(v.as_str().unwrap_or("ASK_FOR_EACH_FILE").to_string()),
        }
    }

    pub fn set_if_file_exists_action(&self, value: &str) -> anyhow::Result<bool> {
        self.set(IF_FILE_EXISTS_ACTION_KEY, value)
    }

    pub fn get_auto_start_download_option(&self) -> anyhow::Result<String> {
        match self.get(AUTO_START_DOWNLOAD_OPTION_KEY)? {
            Value::String(s) => Ok(s),
            v => Ok(v
                .as_str()
                .unwrap_or("ONLY_IF_EXIT_WITH_RUNNING_DOWNLOADS")
                .to_string()),
        }
    }

    pub fn set_auto_start_download_option(&self, value: &str) -> anyhow::Result<bool> {
        self.set(AUTO_START_DOWNLOAD_OPTION_KEY, value)
    }

    pub fn get_show_countdown_on_auto_start_downloads(&self) -> anyhow::Result<bool> {
        Ok(self
            .get(SHOW_COUNTDOWN_ON_AUTO_START_DOWNLOADS_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn set_show_countdown_on_auto_start_downloads(
        &self,
        value: bool,
    ) -> anyhow::Result<bool> {
        self.set(SHOW_COUNTDOWN_ON_AUTO_START_DOWNLOADS_KEY, value)
    }

    pub fn get_auto_start_countdown_seconds(&self) -> anyhow::Result<i32> {
        Ok(self.get(AUTO_START_COUNTDOWN_SECONDS_KEY)?.as_i64().unwrap_or(10) as i32)
    }

    pub fn set_auto_start_countdown_seconds(&self, value: i32) -> anyhow::Result<bool> {
        self.set(AUTO_START_COUNTDOWN_SECONDS_KEY, value)
    }

    pub fn get_hash_check_enabled(&self) -> anyhow::Result<bool> {
        Ok(self.get(HASH_CHECK_ENABLED_KEY)?.as_bool().unwrap_or(true))
    }

    pub fn set_hash_check_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.set(HASH_CHECK_ENABLED_KEY, value)
    }

    pub fn get_hash_retry_enabled(&self) -> anyhow::Result<bool> {
        Ok(self.get(HASH_RETRY_ENABLED_KEY)?.as_bool().unwrap_or(false))
    }

    pub fn set_hash_retry_enabled(&self, value: bool) -> anyhow::Result<bool> {
        self.set(HASH_RETRY_ENABLED_KEY, value)
    }

    pub fn get_auto_open_container_after_download(&self) -> anyhow::Result<bool> {
        Ok(self
            .get(AUTO_OPEN_CONTAINER_AFTER_DOWNLOAD_KEY)?
            .as_bool()
            .unwrap_or(false))
    }

    pub fn set_auto_open_container_after_download(&self, value: bool) -> anyhow::Result<bool> {
        self.set(AUTO_OPEN_CONTAINER_AFTER_DOWNLOAD_KEY, value)
    }

    pub fn get_use_available_accounts(&self) -> anyhow::Result<bool> {
        Ok(self
            .get(USE_AVAILABLE_ACCOUNTS_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    pub fn set_use_available_accounts(&self, value: bool) -> anyhow::Result<bool> {
        self.set(USE_AVAILABLE_ACCOUNTS_KEY, value)
    }

    /// Whether a global download speed cap is active
    /// (`GeneralSettings.isDownloadSpeedLimitEnabled`). Drives the
    /// Speed column's red text in the downloads list, mirroring
    /// `org.jdownloader.gui.views.downloads.columns.SpeedColumn`'s
    /// `configureRendererComponent` — JD colors that column red whenever
    /// this is on, regardless of the actual speed value.
    pub fn get_download_speed_limit_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .get(DOWNLOAD_SPEED_LIMIT_ENABLED_KEY)?
            .as_bool()
            .unwrap_or(false))
    }
}
