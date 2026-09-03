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

use adw::prelude::*;
use gtk4::gio;

/// Sends a notification to the user.
///
/// If the main window is visible, shows an in-app `adw::Toast`.
/// Otherwise sends a GNOME system notification via `gio::Notification`.
pub fn send(
    window: &adw::ApplicationWindow,
    app: &adw::Application,
    toast_overlay: &adw::ToastOverlay,
    title: &str,
    body: &str,
) {
    if window.is_visible() {
        let toast = adw::Toast::new(&format!("{}: {}", title, body));
        toast_overlay.add_toast(toast);
    } else {
        let notif = gio::Notification::new(title);
        notif.set_body(Some(body));
        notif.set_icon(&gio::ThemedIcon::new("io.github.xfuentes.gdownloader"));
        app.send_notification(Some("gdownloader"), &notif);
    }
}

/// Like `send`, but attaches a button label + callback on the toast (window visible only).
pub fn send_with_action(
    window: &adw::ApplicationWindow,
    app: &adw::Application,
    toast_overlay: &adw::ToastOverlay,
    notification_id: &str,
    title: &str,
    body: &str,
    button_label: &str,
    on_button: impl Fn() + 'static,
) {
    if window.is_visible() {
        let toast = adw::Toast::new(&format!("{}: {}", title, body));
        toast.set_button_label(Some(button_label));
        toast.connect_button_clicked(move |_| on_button());
        toast_overlay.add_toast(toast);
    } else {
        let notif = gio::Notification::new(title);
        notif.set_body(Some(body));
        notif.set_icon(&gio::ThemedIcon::new("io.github.xfuentes.gdownloader"));
        app.send_notification(Some(notification_id), &notif);
    }
}
