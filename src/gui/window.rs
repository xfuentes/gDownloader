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

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use adw::prelude::*;
use gtk4::glib;

use crate::jd::{JdApi, JdProcess};

use super::dialogs::WaitDialog;

const SPLASH_IMAGE_NAME: &str = "gDownloader_splash.png";

pub struct SplashWindow {
    pub window: gtk4::Window,
    pub progress: gtk4::ProgressBar,
}

/// Registers the JDownloader image directories into the GTK icon theme.
pub fn setup_icon_theme() {
    let mut candidates = vec![
        PathBuf::from("data/resources/jd/images"),
        PathBuf::from("data/resources/jd/images/logo"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("data/resources/jd/images"));
            candidates.push(exe_dir.join("data/resources/jd/images/logo"));
            candidates.push(exe_dir.join("../share/gdownloader/data/resources/jd/images"));
            candidates.push(exe_dir.join("../share/gdownloader/data/resources/jd/images/logo"));
        }
    }
    if let Some(display) = gtk4::gdk::Display::default() {
        let theme = gtk4::IconTheme::for_display(&display);
        for c in candidates {
            if c.is_dir() {
                theme.add_search_path(&c);
            }
        }
    }
}

/// Injects application-level CSS rules.
pub fn setup_css() {
    let tab_bar_css = gtk4::CssProvider::new();
    tab_bar_css.load_from_string(".tabbar { padding-left: 0; padding-right: 0; }");
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &tab_bar_css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // Compact UI: slightly smaller text and tighter widget padding across the
    // whole app. Uses `em` (relative to the inherited font size) and GTK's
    // own logical-pixel units, both of which are already scaled per-monitor
    // by the desktop's HiDPI/scale-factor setting, so this stays legible on
    // 4K displays without needing to read the DPI ourselves.
    let compact_css = gtk4::CssProvider::new();
    compact_css.load_from_string(
        "window { font-size: 0.92em; }\n\
         button, entry, dropdown { min-height: 22px; padding: 2px 6px; }\n\
         dropdown { margin: 0; padding: 0px; }\n\
         dropdown button, dropdown box, dropdown arrow { min-height: 0; margin: 0; padding: 2px 6px; }\n\
         dropdown button.toggle { padding: 0 0 1px 0; }\n\
         columnview > listview > row { min-height: 24px; }\n\
         columnview > listview > row > cell { padding: 1px; }\n\
         columnview header button { min-height: 22px; padding: 2px 6px; }\n\
         switch { min-width: 34px; min-height: 18px; padding: 1px; }\n\
         switch slider { min-width: 16px; min-height: 16px; }\n\
         spinbutton { min-height: 0; padding: 0; }\n\
         spinbutton text { min-height: 18px; padding: 2px 4px; }\n\
         spinbutton button { min-width: 18px; min-height: 18px; padding: 0; }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &compact_css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    super::cells::progress_cell::install_css();
}

/// Creates the main application window.
pub fn create_main_window(
    app: &adw::Application,
    title: &str,
    is_dev: bool,
) -> adw::ApplicationWindow {
    gtk4::Window::set_default_icon_name("io.github.xfuentes.gdownloader");
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(title)
        .default_width(960)
        .default_height(700)
        .build();
    if is_dev {
        window.add_css_class("devel");
    }
    window
}

/// Creates the splash screen window with its progress bar.
pub fn create_splash(app: &adw::Application) -> SplashWindow {
    let splash_image_path = find_splash_image();
    let picture = if let Some(path) = splash_image_path {
        let p = gtk4::Picture::for_filename(&path);
        p.set_can_shrink(true);
        p.set_content_fit(gtk4::ContentFit::Cover);
        p.set_halign(gtk4::Align::Fill);
        p.set_valign(gtk4::Align::Fill);
        p.set_vexpand(true);
        p
    } else {
        gtk4::Picture::new()
    };

    let progress = gtk4::ProgressBar::new();
    progress.set_show_text(false);
    progress.set_pulse_step(0.1);
    progress.set_halign(gtk4::Align::Fill);
    progress.set_valign(gtk4::Align::End);
    progress.set_margin_start(40);
    progress.set_margin_end(40);
    progress.set_margin_bottom(40);

    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&progress);

    let window = gtk4::Window::builder()
        .application(app)
        .title("gDownloader")
        .default_width(480)
        .default_height(350)
        .resizable(false)
        .decorated(false)
        .build();
    window.set_child(Some(&overlay));

    SplashWindow { window, progress }
}

/// Wires up the main window's close button to gracefully shut down JDownloader.
pub fn setup_close_handler(
    window: &adw::ApplicationWindow,
    api: Arc<JdApi>,
    process: Arc<Mutex<JdProcess>>,
) {
    let closing = Rc::new(Cell::new(false));
    window.connect_close_request({
        let window = window.clone();
        move |_| {
            if closing.get() {
                return glib::Propagation::Proceed;
            }
            closing.set(true);

            let message = tr!("Waiting for JDownloader to exit...");
            let wait = WaitDialog::show(&window, message.as_ref());

            let (tx, rx) = async_channel::bounded(1);
            let api_thread = api.clone();
            let process_thread = process.clone();
            std::thread::spawn(move || {
                let _ = api_thread.system_exit();
                if let Ok(mut p) = process_thread.lock() {
                    let _ = p.stop(true);
                }
                let _ = tx.try_send(());
            });

            let window2 = window.clone();
            glib::MainContext::default().spawn_local(async move {
                let _ = rx.recv().await;
                wait.close();
                window2.close();
            });

            glib::Propagation::Stop
        }
    });
}

fn find_splash_image() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("data/resources").join(SPLASH_IMAGE_NAME)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("data/resources").join(SPLASH_IMAGE_NAME));
            candidates
                .push(exe_dir.join("../share/gdownloader/data/resources").join(SPLASH_IMAGE_NAME));
        }
    }
    for c in candidates {
        if c.exists() {
            return Some(c);
        }
    }
    None
}
