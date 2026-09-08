use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;

use crate::jd::JdApi;

/// A dialog JDownloader wants to show but can't — it runs headless (no
/// local Swing/AWT display), since gDownloader provides the GTK UI
/// instead — so it queues the dialog and blocks the requesting thread
/// until answered via this RemoteAPI instead of showing it locally.
/// Mirrors `org.jdownloader.api.dialog.DialogApiInterface`'s `get()`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PendingDialogInfo {
    /// The dialog's interface class name (e.g.
    /// `org.appwork.uio.ConfirmDialogInterface`) — not currently surfaced
    /// in gDownloader's UI, since a single generic Allow/Deny dialog
    /// handles every known dialog type so far.
    #[allow(dead_code)]
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
}
