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

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use adw::prelude::*;
use gtk4::glib;

use crate::jd::JdApi;

use super::tray::GDownloaderTray;

/// Sets up clipboard monitoring: detects URLs copied by the user, adds them to
/// JDownloader's link collector, and notifies via toast or tray blink.
///
/// Returns a channel sender that can be used to disable/enable clipboard
/// monitoring from outside (mirrors the toggle button state).
pub fn setup(
    api: &Arc<JdApi>,
    clipboard_toggle: &gtk4::ToggleButton,
    window: &adw::ApplicationWindow,
    app: &adw::Application,
    toast_overlay: &adw::ToastOverlay,
    tab_view: &adw::TabView,
    collector_tab: &adw::TabPage,
    tray_handle: &Arc<Mutex<Option<ksni::blocking::Handle<GDownloaderTray>>>>,
) {
    let (toast_tx, toast_rx) = async_channel::unbounded::<(usize, String)>();
    let last_clipboard = Rc::new(RefCell::new(String::new()));
    let display = gtk4::gdk::Display::default();

    // Seed `last_clipboard` with whatever's on the clipboard right now, so
    // pre-existing content isn't treated as newly copied the moment
    // monitoring starts (mirrors JDownloader's own clipboard watcher).
    if let Some(display) = &display {
        let last_clipboard = last_clipboard.clone();
        display
            .clipboard()
            .read_text_async(gtk4::gio::Cancellable::NONE, move |res| {
                if let Ok(Some(text)) = res {
                    last_clipboard.replace(text.to_string());
                }
            });
    }

    // Reads the clipboard and, if its text differs from `last_clipboard`,
    // extracts links, notifies, and adds them. Shared by the "clipboard
    // content changed" signal and by re-enabling monitoring (see below).
    let check_clipboard: Rc<dyn Fn()> = {
        let api = Arc::clone(api);
        let last_clipboard = last_clipboard.clone();
        let toast_tx = toast_tx.clone();
        let tray_handle = Arc::clone(tray_handle);
        let window = window.clone();
        let display = display.clone();
        Rc::new(move || {
            let Some(display) = &display else {
                return;
            };
            let api = api.clone();
            let last_clipboard = last_clipboard.clone();
            let toast_tx = toast_tx.clone();
            let window_for_tray = window.clone();
            let tray_handle_for_tray = tray_handle.clone();
            display
                .clipboard()
                .read_text_async(gtk4::gio::Cancellable::NONE, move |res| {
                    log::info!(
                        "read_text_async completed: {:?}",
                        res.as_ref().map(|o| o.as_ref().map(|s| &s[..s.len().min(80)]))
                    );
                    let Ok(Some(text)) = res else {
                        return;
                    };
                    if text == *last_clipboard.borrow() {
                        log::info!("Clipboard text unchanged, ignoring");
                        return;
                    }
                    last_clipboard.replace(text.to_string());
                    let links = extract_links(&text);
                    log::info!("Extracted {} bytes of links", links.len());
                    if links.is_empty() {
                        return;
                    }
                    let host = links
                        .split("\r\n")
                        .next()
                        .and_then(|l| l.split('/').nth(2))
                        .map(str::to_string)
                        .unwrap_or_default();
                    log::info!("Clipboard captured links from {}", host);
                    let api = api.clone();
                    // Captured now (not re-checked after the wait below):
                    // `ApplicationWindow` isn't `Send`, so it can't be read
                    // from the background thread.
                    let window_hidden = !window_for_tray.is_visible();
                    std::thread::spawn(move || {
                        let job_id = match api.add_links(&links) {
                            Ok(id) => id,
                            Err(e) => {
                                log::warn!("add_links failed: {}", e);
                                return;
                            }
                        };
                        if job_id == 0 {
                            log::warn!("add_links returned no job id, can't tell what was actually added");
                            return;
                        }
                        // Wait for crawling (container/redirect resolution,
                        // dupe-checking) to settle before counting: links
                        // JDownloader already had are dropped during this
                        // phase and never end up queryable by job id.
                        let start = std::time::Instant::now();
                        while start.elapsed() < Duration::from_secs(30) {
                            std::thread::sleep(Duration::from_millis(500));
                            if !api.is_collecting().unwrap_or(false) {
                                break;
                            }
                        }
                        let added = api
                            .query_linkcollector_links_for_job(job_id)
                            .map(|links| links.len())
                            .unwrap_or(0);
                        if added == 0 {
                            log::info!("Clipboard links were all duplicates, not notifying");
                            return;
                        }
                        log::info!("Clipboard added {} link(s) from {}", added, host);
                        if let Err(e) = toast_tx.try_send((added, host)) {
                            log::warn!("toast send failed: {}", e);
                        }
                        // If the window is hidden, blink the tray icon.
                        if window_hidden {
                            if let Some(handle) = tray_handle_for_tray.lock().unwrap().clone() {
                                let _ = handle.update(|tray| {
                                    tray.trigger_alert(Duration::from_secs(3));
                                });
                                for _ in 0..10 {
                                    std::thread::sleep(Duration::from_millis(300));
                                    if !handle.update(|tray| tray.tick_alert()).unwrap_or(false) {
                                        break;
                                    }
                                }
                            }
                        }
                    });
                });
        })
    };

    if let Some(display) = &display {
        let clipboard_toggle = clipboard_toggle.clone();
        let check_clipboard = check_clipboard.clone();
        display.clipboard().connect_changed(move |_cb| {
            log::info!(
                "Clipboard changed; monitoring active={}",
                clipboard_toggle.is_active()
            );
            if clipboard_toggle.is_active() {
                check_clipboard();
            }
        });
    }

    // Disabling monitoring forgets the last-seen clipboard content;
    // re-enabling it re-checks the clipboard against that blank memory, so
    // whatever's on it is treated as freshly copied (mirrors JDownloader:
    // toggling the clipboard watcher off and back on always re-evaluates
    // current content).
    clipboard_toggle.connect_toggled({
        let last_clipboard = last_clipboard.clone();
        let check_clipboard = check_clipboard.clone();
        move |btn| {
            if btn.is_active() {
                check_clipboard();
            } else {
                last_clipboard.replace(String::new());
            }
        }
    });

    // Toast/notification receiver loop.
    {
        let window = window.clone();
        let app = app.clone();
        let tab_view = tab_view.clone();
        let collector_tab = collector_tab.clone();
        let toast_overlay = toast_overlay.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok((count, host)) = toast_rx.recv().await {
                let title = if host.is_empty() {
                    tr!("New links from clipboard")
                } else {
                    tr!("New links from clipboard ({})", host)
                };
                let body = tr!("One link captured" | "{n} links captured" % count);
                log::info!(
                    "Showing notification: visible={}, title={}",
                    window.is_visible(),
                    title
                );
                super::notifications::send_with_action(
                    &window,
                    &app,
                    &toast_overlay,
                    "clipboard-links",
                    title.as_ref(),
                    body.as_ref(),
                    tr!("Show").as_ref(),
                    {
                        let window = window.clone();
                        let tab_view = tab_view.clone();
                        let collector_tab = collector_tab.clone();
                        move || {
                            tab_view.set_selected_page(&collector_tab);
                            window.present();
                        }
                    },
                );
            }
        });
    }
}

/// Extracts whitespace/comma-separated HTTP(S)/FTP URLs from text.
///
/// Returns the links joined by CRLF, ready for JDownloader's `addLinks` call.
pub fn extract_links(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| {
            s.starts_with("http://")
                || s.starts_with("https://")
                || s.starts_with("ftp://")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}
