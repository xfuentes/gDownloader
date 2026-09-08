use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::jd::JdApi;

const INTERFACE: &str = "org.jdownloader.extensions.eventscripter.EventScripterConfig";
const SCRIPTS_KEY: &str = "Scripts";
const API_PANEL_VISIBLE_KEY: &str = "APIPanelVisible";

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Mirrors `org.jdownloader.extensions.eventscripter.EventTrigger`. Variant
/// order matches `EventTrigger.values()` (used to populate the trigger
/// chooser, same as JDownloader's own combo box).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventTrigger {
    #[serde(rename = "ON_DOWNLOAD_CONTROLLER_START")]
    OnDownloadControllerStart,
    #[serde(rename = "ON_DOWNLOAD_CONTROLLER_STOPPED")]
    OnDownloadControllerStopped,
    #[serde(rename = "ON_PACKAGE_FINISHED")]
    OnPackageFinished,
    #[serde(rename = "ON_GENERIC_EXTRACTION")]
    OnGenericExtraction,
    #[serde(rename = "ON_ARCHIVE_EXTRACTED")]
    OnArchiveExtracted,
    #[serde(rename = "ON_JDOWNLOADER_STARTED")]
    OnJdownloaderStarted,
    #[default]
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "ON_OUTGOING_REMOTE_API_EVENT")]
    OnOutgoingRemoteApiEvent,
    #[serde(rename = "ON_NEW_FILE")]
    OnNewFile,
    #[serde(rename = "ON_NEW_CRAWLER_JOB")]
    OnNewCrawlerJob,
    #[serde(rename = "ON_FINISHED_CRAWLER_JOB")]
    OnFinishedCrawlerJob,
    #[serde(rename = "ON_NEW_LINK")]
    OnNewLink,
    #[serde(rename = "ON_PACKAGIZER")]
    OnPackagizer,
    #[serde(rename = "ON_DOWNLOADS_PAUSE")]
    OnDownloadsPause,
    #[serde(rename = "ON_DOWNLOADS_RUNNING")]
    OnDownloadsRunning,
    #[serde(rename = "ON_DOWNLOADS_STOPPED")]
    OnDownloadsStopped,
    #[serde(rename = "RECONNECT_BEFORE")]
    ReconnectBefore,
    #[serde(rename = "RECONNECT_AFTER")]
    ReconnectAfter,
    #[serde(rename = "CAPTCHA_CHALLENGE_BEFORE")]
    CaptchaChallengeBefore,
    #[serde(rename = "CAPTCHA_CHALLENGE_AFTER")]
    CaptchaChallengeAfter,
    #[serde(rename = "INTERVAL")]
    Interval,
    #[serde(rename = "TOOLBAR_BUTTON")]
    ToolbarButton,
    #[serde(rename = "MAIN_MENU_BUTTON")]
    MainMenuButton,
    #[serde(rename = "DOWNLOAD_TABLE_CONTEXT_MENU_BUTTON")]
    DownloadTableContextMenuButton,
    #[serde(rename = "LINKGRABBER_TABLE_CONTEXT_MENU_BUTTON")]
    LinkgrabberTableContextMenuButton,
    #[serde(rename = "DOWNLOAD_TABLE_BOTTOM_BAR_BUTTON")]
    DownloadTableBottomBarButton,
    #[serde(rename = "LINKGRABBER_BOTTOM_BAR_BUTTON")]
    LinkgrabberBottomBarButton,
    #[serde(rename = "TRAY_BUTTON")]
    TrayButton,
}

impl EventTrigger {
    pub const ALL: [EventTrigger; 28] = [
        EventTrigger::OnDownloadControllerStart,
        EventTrigger::OnDownloadControllerStopped,
        EventTrigger::OnPackageFinished,
        EventTrigger::OnGenericExtraction,
        EventTrigger::OnArchiveExtracted,
        EventTrigger::OnJdownloaderStarted,
        EventTrigger::None,
        EventTrigger::OnOutgoingRemoteApiEvent,
        EventTrigger::OnNewFile,
        EventTrigger::OnNewCrawlerJob,
        EventTrigger::OnFinishedCrawlerJob,
        EventTrigger::OnNewLink,
        EventTrigger::OnPackagizer,
        EventTrigger::OnDownloadsPause,
        EventTrigger::OnDownloadsRunning,
        EventTrigger::OnDownloadsStopped,
        EventTrigger::ReconnectBefore,
        EventTrigger::ReconnectAfter,
        EventTrigger::CaptchaChallengeBefore,
        EventTrigger::CaptchaChallengeAfter,
        EventTrigger::Interval,
        EventTrigger::ToolbarButton,
        EventTrigger::MainMenuButton,
        EventTrigger::DownloadTableContextMenuButton,
        EventTrigger::LinkgrabberTableContextMenuButton,
        EventTrigger::DownloadTableBottomBarButton,
        EventTrigger::LinkgrabberBottomBarButton,
        EventTrigger::TrayButton,
    ];

    /// English label. JDownloader's own translations for these keys were not
    /// available to port verbatim; these mirror the trigger's meaning.
    pub fn label(&self) -> &'static str {
        match self {
            EventTrigger::OnDownloadControllerStart => "Download controller started",
            EventTrigger::OnDownloadControllerStopped => "Download controller stopped",
            EventTrigger::OnPackageFinished => "Package finished",
            EventTrigger::OnGenericExtraction => "Extraction event",
            EventTrigger::OnArchiveExtracted => "Archive extracted",
            EventTrigger::OnJdownloaderStarted => "JDownloader started",
            EventTrigger::None => "None",
            EventTrigger::OnOutgoingRemoteApiEvent => "Outgoing RemoteAPI event",
            EventTrigger::OnNewFile => "New file",
            EventTrigger::OnNewCrawlerJob => "New crawler job",
            EventTrigger::OnFinishedCrawlerJob => "Crawler job finished",
            EventTrigger::OnNewLink => "New link",
            EventTrigger::OnPackagizer => "Packagizer",
            EventTrigger::OnDownloadsPause => "Downloads paused",
            EventTrigger::OnDownloadsRunning => "Downloads running",
            EventTrigger::OnDownloadsStopped => "Downloads stopped",
            EventTrigger::ReconnectBefore => "Before reconnect",
            EventTrigger::ReconnectAfter => "After reconnect",
            EventTrigger::CaptchaChallengeBefore => "Before captcha challenge",
            EventTrigger::CaptchaChallengeAfter => "After captcha challenge",
            EventTrigger::Interval => "Interval (timer)",
            EventTrigger::ToolbarButton => "Toolbar button",
            EventTrigger::MainMenuButton => "Main menu button",
            EventTrigger::DownloadTableContextMenuButton => "Download list context menu button",
            EventTrigger::LinkgrabberTableContextMenuButton => "Linkgrabber context menu button",
            EventTrigger::DownloadTableBottomBarButton => "Download list bottom bar button",
            EventTrigger::LinkgrabberBottomBarButton => "Linkgrabber bottom bar button",
            EventTrigger::TrayButton => "Tray menu button",
        }
    }

    /// Mirrors `EventTrigger.isSynchronousSupported()`.
    pub fn supports_synchronous(&self) -> bool {
        matches!(
            self,
            EventTrigger::OnDownloadControllerStart
                | EventTrigger::OnDownloadControllerStopped
                | EventTrigger::OnPackageFinished
                | EventTrigger::OnGenericExtraction
                | EventTrigger::OnArchiveExtracted
                | EventTrigger::OnNewCrawlerJob
                | EventTrigger::OnNewLink
                | EventTrigger::OnPackagizer
                | EventTrigger::ReconnectBefore
                | EventTrigger::ReconnectAfter
                | EventTrigger::CaptchaChallengeBefore
                | EventTrigger::CaptchaChallengeAfter
                | EventTrigger::Interval
        )
    }

    /// Mirrors `EventTrigger.isDefaultSynchronous()`.
    pub fn default_synchronous(&self) -> bool {
        matches!(
            self,
            EventTrigger::OnNewCrawlerJob
                | EventTrigger::OnNewLink
                | EventTrigger::OnPackagizer
                | EventTrigger::ReconnectBefore
                | EventTrigger::ReconnectAfter
                | EventTrigger::CaptchaChallengeBefore
                | EventTrigger::CaptchaChallengeAfter
        )
    }

    pub fn is_interval(&self) -> bool {
        matches!(self, EventTrigger::Interval)
    }
}

/// Mirrors `org.jdownloader.extensions.eventscripter.ScriptEntry`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptEntry {
    #[serde(default = "now_millis", rename = "ID")]
    pub id: i64,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default, rename = "eventTrigger")]
    pub event_trigger: EventTrigger,
    /// Trigger-specific settings (e.g. `interval` for [`EventTrigger::Interval`],
    /// `isSynchronous`, or menu placement metadata for button triggers).
    /// Kept as a raw map so fields gDownloader does not edit round-trip
    /// losslessly with real JDownloader.
    #[serde(default, rename = "eventTriggerSettings")]
    pub event_trigger_settings: Map<String, Value>,
}

impl ScriptEntry {
    pub fn new(name: &str) -> Self {
        Self {
            id: now_millis(),
            enabled: true,
            name: Some(name.to_string()),
            script: None,
            event_trigger: EventTrigger::default(),
            event_trigger_settings: Map::new(),
        }
    }

    pub fn is_synchronous(&self) -> bool {
        if !self.event_trigger.supports_synchronous() {
            return false;
        }
        match self.event_trigger_settings.get("isSynchronous") {
            Some(Value::Bool(b)) => *b,
            Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
            _ => self.event_trigger.default_synchronous(),
        }
    }

    pub fn set_synchronous(&mut self, value: bool) {
        if self.event_trigger.supports_synchronous() {
            self.event_trigger_settings
                .insert("isSynchronous".to_string(), Value::Bool(value));
        } else {
            self.event_trigger_settings.remove("isSynchronous");
        }
    }

    pub fn interval_ms(&self) -> i64 {
        match self.event_trigger_settings.get("interval") {
            Some(v) => v.as_i64().unwrap_or(1000),
            None => 1000,
        }
    }

    pub fn set_interval_ms(&mut self, value: i64) {
        self.event_trigger_settings
            .insert("interval".to_string(), Value::from(value));
    }
}

/// Accessor for JDownloader's `EventScripterConfig` (the "Scripts" /
/// "Eventscripter" automation feature).
#[derive(Clone)]
pub struct EventScripterSettings {
    api: Arc<JdApi>,
}

impl EventScripterSettings {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn is_ready(&self) -> bool {
        self.api.is_ready()
    }

    pub fn get_scripts(&self) -> anyhow::Result<Vec<ScriptEntry>> {
        let value = self.api.get_config(INTERFACE, SCRIPTS_KEY)?;
        if value.is_null() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_value(value)?)
    }

    pub fn set_scripts(&self, scripts: &[ScriptEntry]) -> anyhow::Result<bool> {
        let value = serde_json::to_value(scripts)?;
        self.api.set_config(INTERFACE, SCRIPTS_KEY, &value)
    }

    #[allow(dead_code)]
    pub fn get_api_panel_visible(&self) -> anyhow::Result<bool> {
        Ok(self
            .api
            .get_config(INTERFACE, API_PANEL_VISIBLE_KEY)?
            .as_bool()
            .unwrap_or(true))
    }

    #[allow(dead_code)]
    pub fn set_api_panel_visible(&self, value: bool) -> anyhow::Result<bool> {
        let v = serde_json::to_value(value)?;
        self.api.set_config(INTERFACE, API_PANEL_VISIBLE_KEY, &v)
    }
}
