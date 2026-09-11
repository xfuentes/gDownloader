use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::gio;

use super::{action_button, custom_item, disabled_button, MenuChildren};
use crate::gui::download_limits::{DownloadLimits, DownloadLimitsCache};
use crate::gui::icon_key;

pub fn build(download_limits: DownloadLimitsCache) -> (gio::Menu, MenuChildren) {
    let settings_menu = gio::Menu::new();
    let settings_top_section = gio::Menu::new();
    settings_top_section.append_item(&custom_item(
        tr!("Settings").as_ref(),
        Some("win.settings"),
        "settings-settings",
    ));
    settings_top_section.append_item(&custom_item(
        tr!("My.JDownloader").as_ref(),
        None,
        "settings-myjd",
    ));
    settings_menu.append_section(None, &settings_top_section);

    let settings_tuning_section = gio::Menu::new();
    settings_tuning_section.append_item(&custom_item(
        tr!("Max. chunks per download").as_ref(),
        None,
        "settings-chunks",
    ));
    settings_tuning_section.append_item(&custom_item(
        tr!("Max. simultaneous downloads").as_ref(),
        None,
        "settings-sim",
    ));
    settings_tuning_section.append_item(&custom_item(
        tr!("Max. sim. Downloads per Hoster").as_ref(),
        None,
        "settings-host",
    ));
    settings_tuning_section.append_item(&custom_item(
        tr!("Speed limit (KiB/s)").as_ref(),
        None,
        "settings-speed",
    ));
    settings_menu.append_section(None, &settings_tuning_section);

    // Same download-behavior editors as the Downloads list's own Quick
    // Settings menu (JDownloader itself reuses the very same
    // ChunksEditor/ParalellDownloadsEditor/ParallelDownloadsPerHostEditor/
    // SpeedlimitEditor widgets between its main toolbar and its downloads
    // bottom bar — `org.jdownloader.gui.toolbar.MenuManagerMainToolbar` and
    // `MenuManagerDownloadTabBottomBar` both build the same "quick
    // settings" editors). Sharing the same `DownloadLimitsCache` here keeps
    // a change made in one of those places in sync with this one.
    let label_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    let checkbox_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Both);
    let spin_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Both);
    let icon_label_content = |icon: &str, label: &str| -> (gtk4::Box, gtk4::Image) {
        let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let icon_widget = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(icon));
        icon_widget.set_pixel_size(18);
        let label_widget = gtk4::Label::new(Some(label));
        label_widget.set_xalign(0.0);
        content.append(&icon_widget);
        content.append(&label_widget);
        label_size_group.add_widget(&content);
        (content, icon_widget)
    };
    let set_row_enabled = |content: &gtk4::Box, icon: &gtk4::Image, enabled: bool| {
        content.set_sensitive(enabled);
        icon.set_opacity(if enabled { 1.0 } else { 0.5 });
    };

    let chunks_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    chunks_row.add_css_class("menu-editor-row");
    chunks_row.append(&icon_label_content(icon_key::ICON_CHUNKS, tr!("Max. chunks per download").as_ref()).0);
    let chunks_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    checkbox_size_group.add_widget(&chunks_spacer);
    chunks_row.append(&chunks_spacer);
    let chunks_adj = gtk4::Adjustment::new(1.0, 1.0, 20.0, 1.0, 1.0, 0.0);
    let chunks_spin = gtk4::SpinButton::new(Some(&chunks_adj), 1.0, 0);
    spin_size_group.add_widget(&chunks_spin);
    chunks_row.append(&chunks_spin);

    let parallel_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    parallel_row.add_css_class("menu-editor-row");
    parallel_row.append(
        &icon_label_content(icon_key::ICON_PARALELL, tr!("Max. simultaneous downloads").as_ref()).0,
    );
    let parallel_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    checkbox_size_group.add_widget(&parallel_spacer);
    parallel_row.append(&parallel_spacer);
    let parallel_adj = gtk4::Adjustment::new(3.0, 1.0, 20.0, 1.0, 1.0, 0.0);
    let parallel_spin = gtk4::SpinButton::new(Some(&parallel_adj), 1.0, 0);
    spin_size_group.add_widget(&parallel_spin);
    parallel_row.append(&parallel_spin);

    let per_host_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    per_host_row.add_css_class("menu-editor-row");
    let (per_host_content, per_host_icon) =
        icon_label_content(icon_key::ICON_BATCH, tr!("Max. sim. Downloads per Hoster").as_ref());
    set_row_enabled(&per_host_content, &per_host_icon, false);
    per_host_row.append(&per_host_content);
    let per_host_check = gtk4::CheckButton::new();
    checkbox_size_group.add_widget(&per_host_check);
    let per_host_adj = gtk4::Adjustment::new(1.0, 1.0, 40.0, 1.0, 1.0, 0.0);
    let per_host_spin = gtk4::SpinButton::new(Some(&per_host_adj), 1.0, 0);
    per_host_spin.set_sensitive(false);
    spin_size_group.add_widget(&per_host_spin);
    per_host_row.append(&per_host_check);
    per_host_row.append(&per_host_spin);

    let speed_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    speed_row.add_css_class("menu-editor-row");
    let (speed_content, speed_icon) =
        icon_label_content(icon_key::ICON_SPEED, tr!("Speed limit (KiB/s)").as_ref());
    set_row_enabled(&speed_content, &speed_icon, false);
    speed_row.append(&speed_content);
    let speed_check = gtk4::CheckButton::new();
    checkbox_size_group.add_widget(&speed_check);
    let speed_adj = gtk4::Adjustment::new(50.0, 1.0, 1_000_000.0, 1.0, 10.0, 0.0);
    let speed_spin = gtk4::SpinButton::new(Some(&speed_adj), 1.0, 0);
    speed_spin.set_sensitive(false);
    spin_size_group.add_widget(&speed_spin);
    speed_row.append(&speed_check);
    speed_row.append(&speed_spin);

    let loading_limits = Rc::new(Cell::new(true));
    download_limits.subscribe({
        let loading_limits = loading_limits.clone();
        let chunks_spin = chunks_spin.clone();
        let parallel_spin = parallel_spin.clone();
        let per_host_check = per_host_check.clone();
        let per_host_spin = per_host_spin.clone();
        let per_host_content = per_host_content.clone();
        let per_host_icon = per_host_icon.clone();
        let speed_check = speed_check.clone();
        let speed_spin = speed_spin.clone();
        let speed_content = speed_content.clone();
        let speed_icon = speed_icon.clone();
        move |limits: DownloadLimits| {
            loading_limits.set(true);
            chunks_spin.set_value(limits.max_chunks as f64);
            parallel_spin.set_value(limits.max_simultaneous as f64);
            per_host_check.set_active(limits.max_simultaneous_per_host_enabled);
            per_host_spin.set_value(limits.max_simultaneous_per_host as f64);
            per_host_spin.set_sensitive(limits.max_simultaneous_per_host_enabled);
            set_row_enabled(
                &per_host_content,
                &per_host_icon,
                limits.max_simultaneous_per_host_enabled,
            );
            speed_check.set_active(limits.speed_limit_enabled);
            speed_spin.set_value((limits.speed_limit / 1024) as f64);
            speed_spin.set_sensitive(limits.speed_limit_enabled);
            set_row_enabled(&speed_content, &speed_icon, limits.speed_limit_enabled);
            loading_limits.set(false);
        }
    });

    chunks_spin.connect_value_changed({
        let download_limits = download_limits.clone();
        let loading_limits = loading_limits.clone();
        move |spin| {
            if loading_limits.get() {
                return;
            }
            let value = spin.value() as i32;
            download_limits.update(|limits| limits.max_chunks = value);
        }
    });
    parallel_spin.connect_value_changed({
        let download_limits = download_limits.clone();
        let loading_limits = loading_limits.clone();
        move |spin| {
            if loading_limits.get() {
                return;
            }
            let value = spin.value() as i32;
            download_limits.update(|limits| limits.max_simultaneous = value);
        }
    });
    per_host_check.connect_toggled({
        let download_limits = download_limits.clone();
        let loading_limits = loading_limits.clone();
        let per_host_spin = per_host_spin.clone();
        let per_host_content = per_host_content.clone();
        let per_host_icon = per_host_icon.clone();
        move |btn| {
            let value = btn.is_active();
            per_host_spin.set_sensitive(value);
            set_row_enabled(&per_host_content, &per_host_icon, value);
            if loading_limits.get() {
                return;
            }
            download_limits.update(|limits| limits.max_simultaneous_per_host_enabled = value);
        }
    });
    per_host_spin.connect_value_changed({
        let download_limits = download_limits.clone();
        let loading_limits = loading_limits.clone();
        move |spin| {
            if loading_limits.get() {
                return;
            }
            let value = spin.value() as i32;
            download_limits.update(|limits| limits.max_simultaneous_per_host = value);
        }
    });
    speed_check.connect_toggled({
        let download_limits = download_limits.clone();
        let loading_limits = loading_limits.clone();
        let speed_spin = speed_spin.clone();
        let speed_content = speed_content.clone();
        let speed_icon = speed_icon.clone();
        move |btn| {
            let value = btn.is_active();
            speed_spin.set_sensitive(value);
            set_row_enabled(&speed_content, &speed_icon, value);
            if loading_limits.get() {
                return;
            }
            download_limits.update(|limits| limits.speed_limit_enabled = value);
        }
    });
    speed_spin.connect_value_changed({
        let download_limits = download_limits.clone();
        let loading_limits = loading_limits.clone();
        move |spin| {
            if loading_limits.get() {
                return;
            }
            let value_bytes = (spin.value() as i32).saturating_mul(1024);
            download_limits.update(|limits| limits.speed_limit = value_bytes);
        }
    });

    let children: MenuChildren = vec![
        (
            action_button("settings", tr!("Settings").as_ref(), "win.settings").upcast(),
            "settings-settings".to_string(),
        ),
        (
            disabled_button("myjdownloader", tr!("My.JDownloader").as_ref()).upcast(),
            "settings-myjd".to_string(),
        ),
        (chunks_row.upcast(), "settings-chunks".to_string()),
        (parallel_row.upcast(), "settings-sim".to_string()),
        (per_host_row.upcast(), "settings-host".to_string()),
        (speed_row.upcast(), "settings-speed".to_string()),
    ];

    (settings_menu, children)
}
