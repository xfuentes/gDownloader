use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;

use crate::jd::JdApi;

/// Interface class name of JDownloader's "file already exists" dialog
/// (`jd.controlling.downloadcontroller.IfFileExistsDialogInterface`) — unlike
/// the generic `ConfirmDialogInterface` (a plain OK/Cancel prompt), answering
/// this one requires an extra `action` field alongside `closereason`/
/// `dontshowagainselected` (see [`FileExistsAction`]); without it JDownloader
/// treats the answer as if `action` were unset and silently falls back to
/// skipping the file (`DownloadWatchDog.fileAlreadyExistsHandling`), no
/// matter which button was clicked.
const IF_FILE_EXISTS_TYPE: &str = "jd.controlling.downloadcontroller.IfFileExistsDialogInterface";

/// A dialog JDownloader wants to show but can't — it runs headless (no
/// local Swing/AWT display), since gDownloader provides the GTK UI
/// instead — so it queues the dialog and blocks the requesting thread
/// until answered via this RemoteAPI instead of showing it locally.
/// Mirrors `org.jdownloader.api.dialog.DialogApiInterface`'s `get()`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PendingDialogInfo {
    /// The dialog's interface class name (e.g.
    /// `org.appwork.uio.ConfirmDialogInterface`). Most dialog types are
    /// handled by a single generic Allow/Deny prompt regardless of this
    /// value, except [`IF_FILE_EXISTS_TYPE`] (see [`Self::is_file_exists`]),
    /// which needs its own UI and answer shape.
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

impl PendingDialogInfo {
    pub fn title(&self) -> &str {
        self.properties
            .get("title")
            .map(String::as_str)
            .unwrap_or_default()
    }

    pub fn message(&self) -> &str {
        self.properties
            .get("message")
            .map(String::as_str)
            .unwrap_or_default()
    }

    pub fn is_file_exists(&self) -> bool {
        self.type_name == IF_FILE_EXISTS_TYPE
    }

    /// Full destination path of the file being downloaded (only set for
    /// [`Self::is_file_exists`] dialogs).
    pub fn file_path(&self) -> &str {
        self.properties
            .get("filepath")
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Only set for [`Self::is_file_exists`] dialogs.
    pub fn package_name(&self) -> &str {
        self.properties
            .get("packagename")
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// The hoster the conflicting file was downloaded from. Only set for
    /// [`Self::is_file_exists`] dialogs, and only meaningful for a plain
    /// download conflict — JDownloader's extraction-conflict variant of this
    /// same dialog type implements the same `getHost()` accessor but its own
    /// UI never shows it.
    pub fn host(&self) -> &str {
        self.properties
            .get("host")
            .map(String::as_str)
            .unwrap_or_default()
    }
}

/// The user's choice on an [`IF_FILE_EXISTS_TYPE`] dialog — mirrors
/// `org.jdownloader.settings.IfFileExistsAction` (minus `ASK_FOR_EACH_FILE`,
/// which isn't a valid *answer*: it's the settings-panel option that makes
/// JDownloader raise this dialog in the first place).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileExistsAction {
    Overwrite,
    Skip,
    Rename,
}

impl FileExistsAction {
    fn as_jd_value(self) -> &'static str {
        match self {
            FileExistsAction::Overwrite => "OVERWRITE_FILE",
            FileExistsAction::Skip => "SKIP_FILE",
            FileExistsAction::Rename => "AUTO_RENAME",
        }
    }
}

/// Accessor for JDownloader's `dialogs` RemoteAPI — the mechanism that lets
/// a headless JDownloader instance surface dialogs (e.g. the EventScripter
/// "allow this script to run a program?" prompt) for a client to answer
/// remotely, instead of the Swing dialog it would otherwise show.
#[derive(Clone)]
pub struct JdDialogs {
    api: Arc<JdApi>,
}

impl JdDialogs {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    /// IDs of dialogs currently waiting for an answer, oldest first.
    /// JDownloader requires dialogs to be answered in this order — see
    /// [`Self::answer`].
    pub fn list(&self) -> Result<Vec<i64>> {
        let value = self.api.call("dialogs/list", &[])?;
        let mut ids: Vec<i64> = serde_json::from_value(value).unwrap_or_default();
        ids.sort_unstable();
        Ok(ids)
    }

    pub fn get(&self, id: i64) -> Result<PendingDialogInfo> {
        let value = self
            .api
            .call("dialogs/get", &[&id.to_string(), "false", "true"])?;
        Ok(serde_json::from_value(value)?)
    }

    /// Answers a pending dialog. JDownloader rejects answering a dialog
    /// while an older one (lower id) is still pending, so callers must
    /// process [`Self::list`]'s ids in order.
    pub fn answer(&self, id: i64, allow: bool, dont_show_again: bool) -> Result<()> {
        let data = serde_json::json!({
            "closereason": if allow { "OK" } else { "CANCEL" },
            "dontshowagainselected": dont_show_again,
        })
        .to_string();
        self.api.call("dialogs/answer", &[&id.to_string(), &data])?;
        Ok(())
    }

    /// Answers an [`crate::jd::dialogs::PendingDialogInfo::is_file_exists`]
    /// dialog. `allow: false` (the dialog's Cancel button, or closing it any
    /// other way) sends `closereason: "CANCEL"`, which makes JDownloader
    /// throw a `DialogCanceledException` client-side *before* it ever reads
    /// `action` — its own watchdog catches that and falls back to
    /// [`FileExistsAction::Skip`] regardless of what's passed here (see
    /// `DownloadWatchDog.fileAlreadyExistsHandling`), so `action` only
    /// matters when `allow` is `true`.
    pub fn answer_file_exists(
        &self,
        id: i64,
        allow: bool,
        action: FileExistsAction,
        dont_show_again: bool,
    ) -> Result<()> {
        let data = serde_json::json!({
            "closereason": if allow { "OK" } else { "CANCEL" },
            "dontshowagainselected": dont_show_again,
            "action": action.as_jd_value(),
        })
        .to_string();
        self.api.call("dialogs/answer", &[&id.to_string(), &data])?;
        Ok(())
    }
}
