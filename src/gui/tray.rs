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

use std::time::{Duration, Instant};

/// Message sent from the tray icon (its own thread) to the GTK main loop.
pub enum TrayMessage {
    ShowWindow,
    StartDownloads,
    StopDownloads,
    TogglePause,
    OpenDownloadFolder,
    ToggleClipboardMonitoring,
    ToggleAutoReconnect,
    TogglePremium,
    Exit,
}

/// StatusNotifierItem tray icon, mirroring JDownloader's tray menu:
/// a clickable title header on top, then the download actions and Exit.
pub struct GDownloaderTray {
    tx: async_channel::Sender<TrayMessage>,
    alert: bool,
    alert_until: Option<Instant>,
    running: bool,
    paused: bool,
    clipboard_monitored: bool,
    auto_reconnect_enabled: bool,
    auto_reconnect_available: bool,
    use_premium_accounts: bool,
}

impl GDownloaderTray {
    pub fn new(tx: async_channel::Sender<TrayMessage>) -> Self {
        Self {
            tx,
            alert: false,
            alert_until: None,
            running: false,
            paused: false,
            clipboard_monitored: false,
            auto_reconnect_enabled: false,
            auto_reconnect_available: false,
            use_premium_accounts: false,
        }
    }

    fn send(&self, msg: TrayMessage) {
        let _ = self.tx.try_send(msg);
    }

    /// Updates the running/paused state used to show/hide the start/pause/
    /// stop menu entries, and the clipboard/auto-reconnect/premium toggle
    /// states, mirroring the toolbar's own rules (`MainToolBar::wire`'s
    /// status poll).
    #[allow(clippy::too_many_arguments)]
    pub fn set_status(
        &mut self,
        running: bool,
        paused: bool,
        clipboard_monitored: bool,
        auto_reconnect_enabled: bool,
        auto_reconnect_available: bool,
        use_premium_accounts: bool,
    ) {
        self.running = running;
        self.paused = paused;
        self.clipboard_monitored = clipboard_monitored;
        self.auto_reconnect_enabled = auto_reconnect_enabled;
        self.auto_reconnect_available = auto_reconnect_available;
        self.use_premium_accounts = use_premium_accounts;
    }

    /// Start blinking the tray icon for the given duration.
    pub fn trigger_alert(&mut self, duration: Duration) {
        self.alert = true;
        self.alert_until = Some(Instant::now() + duration);
    }

    /// Toggle the alert frame; if the duration elapsed, stop.
    pub fn tick_alert(&mut self) -> bool {
        if let Some(until) = self.alert_until {
            if Instant::now() >= until {
                self.alert = false;
                self.alert_until = None;
                return false;
            }
            self.alert = !self.alert;
            return true;
        }
        false
    }
}

impl ksni::Tray for GDownloaderTray {
    fn id(&self) -> String {
        "io.github.xfuentes.gdownloader".into()
    }

    fn icon_name(&self) -> String {
        if self.alert {
            "emblem-important".into()
        } else {
            "io.github.xfuentes.gdownloader".into()
        }
    }

    fn title(&self) -> String {
        "gDownloader".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayMessage::ShowWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                // DBusMenu only carries plain text: fake bold with Unicode
                // sans-serif bold letters, centering is not supported.
                label: "        𝗴𝗗𝗼𝘄𝗻𝗹𝗼𝗮𝗱𝗲𝗿        ".into(),
                disposition: ksni::menu::Disposition::Informative,
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::ShowWindow)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: tr!("Start downloads").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_MEDIA_PLAYBACK_START),
                visible: !self.running,
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::StartDownloads)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: tr!("Stop downloads").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_MEDIA_PLAYBACK_STOP),
                visible: self.running,
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::StopDownloads)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: tr!("Pause downloads").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_MEDIA_PLAYBACK_PAUSE),
                visible: self.running || self.paused,
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::TogglePause)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                // Mirrors JDownloader's own tray menu entry
                // (`TrayOpenDefaultDownloadDirectory` / `IconKey.ICON_SAVE`).
                label: tr!("Open download folder").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_SAVE),
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::OpenDownloadFolder)),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: tr!("Clipboard monitoring").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_CLIPBOARD),
                checked: self.clipboard_monitored,
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::ToggleClipboardMonitoring)),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: tr!("Auto reconnect").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_AUTO_RECONNECT),
                checked: self.auto_reconnect_enabled,
                enabled: self.auto_reconnect_available,
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::ToggleAutoReconnect)),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: tr!("Use premium accounts").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_PREMIUM),
                checked: self.use_premium_accounts,
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::TogglePremium)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: tr!("Exit").to_string(),
                icon_data: Self::jd_icon_data(crate::gui::icon_key::ICON_EXIT),
                activate: Box::new(|tray: &mut Self| tray.send(TrayMessage::Exit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

impl GDownloaderTray {
    /// Loads a JDownloader theme icon as raw PNG bytes for the DBus menu.
    fn jd_icon_data(name: &str) -> Vec<u8> {
        crate::gui::jd_icon::resolve_path(name)
            .and_then(|path| std::fs::read(path).ok())
            .unwrap_or_default()
    }
}
