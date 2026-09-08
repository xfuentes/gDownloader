use adw::prelude::*;
use gtk4::glib;
use log::warn;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::gui::components::{ConfigSection, ConfigSectionForm};
use crate::gui::jd_icon;
use crate::gui::spawn::{api_call, api_fire};
use crate::jd::{ExtensionQuery, ExtensionStorable, JdApi, JdExtensions, INTERNAL_JD_PORT};

fn wait_ready(extensions: &JdExtensions) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if extensions.is_ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Fetches the extension list, retrying for `retry_for` since JDownloader
/// populates `ExtensionController` asynchronously (its own GUI waits on
/// `SecondLevelLaunch.EXTENSIONS_LOADED`) well after the deprecated API
/// starts responding — an immediate single call can race a still-empty list.
fn load_list(extensions: &JdExtensions, retry_for: Duration) -> Vec<ExtensionStorable> {
    if !wait_ready(extensions) {
        return Vec::new();
    }
    let deadline = Instant::now() + retry_for;
    loop {
        match extensions.list(&ExtensionQuery::all()) {
            Ok(list) if !list.is_empty() => return list,
            Ok(_) => {}
            Err(e) => warn!("extensions/list failed: {}", e),
        }
        if Instant::now() >= deadline {
            return Vec::new();
        }
        thread::sleep(Duration::from_millis(800));
    }
}

pub struct ExtensionManagerPage;

impl ExtensionManagerPage {
    pub fn build() -> gtk4::ScrolledWindow {
        let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
        let extensions = JdExtensions::new(Arc::clone(&api));

        let page_box = gtk4::Box::new(gtk4::Orientation::Vertical, 15);
        page_box.set_margin_top(15);
        page_box.set_margin_bottom(15);
        page_box.set_margin_start(15);
        page_box.set_margin_end(15);
        page_box.set_hexpand(true);
        page_box.set_vexpand(true);

        let label_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let checkbox_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let input_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let empty_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let header_section = ConfigSection::new(
            crate::gui::icon_key::ICON_EXTENSIONMANAGER,
            tr!("Extension Manager").as_ref(),
            Some(tr!("Enable, disable or install JDownloader extensions.").as_ref()),
            &empty_form,
        );

        let refresh_btn = gtk4::Button::from_icon_name(crate::gui::icon_key::ICON_REFRESH);
        refresh_btn.set_tooltip_text(Some(tr!("Refresh").as_ref()));
        refresh_btn.set_valign(gtk4::Align::Start);

        let header_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        header_row.append(header_section.widget());
        header_row.append(&refresh_btn);
        page_box.append(&header_row);

        let list_box = gtk4::ListBox::new();
        list_box.set_selection_mode(gtk4::SelectionMode::None);
        list_box.add_css_class("boxed-list");

        let placeholder = gtk4::Label::new(Some(tr!("Loading extensions…").as_ref()));
        placeholder.add_css_class("dim-label");
        placeholder.set_margin_top(24);
        placeholder.set_margin_bottom(24);
        list_box.set_placeholder(Some(&placeholder));

        let scroll = gtk4::ScrolledWindow::builder()
            .child(&list_box)
            .vexpand(true)
            .hexpand(true)
            .min_content_height(320)
            .propagate_natural_height(true)
            .build();
        page_box.append(&scroll);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page_box));
        scrolled.set_vexpand(true);

        let (tx, rx) = async_channel::bounded::<Vec<ExtensionStorable>>(1);

        let load = {
            let extensions = extensions.clone();
            let tx = tx.clone();
            move |retry_for: Duration| {
                let extensions = extensions.clone();
                let tx = tx.clone();
                thread::spawn(move || {
                    let list = load_list(&extensions, retry_for);
                    let _ = tx.try_send(list);
                });
            }
        };
        load(Duration::from_secs(25));

        let load_c = load.clone();
        refresh_btn.connect_clicked(move |_| load_c(Duration::from_secs(3)));

        let extensions_c = extensions.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(list) = rx.recv().await {
                if list.is_empty() {
                    placeholder.set_label(tr!("No extensions found.").as_ref());
                }
                list_box.remove_all();
                for ext in &list {
                    list_box.append(&build_row(&extensions_c, ext, {
                        let load = load.clone();
                        move || load(Duration::from_secs(3))
                    }));
                }
            }
        });

        scrolled
    }
}

fn build_row(
    extensions: &JdExtensions,
    ext: &ExtensionStorable,
    reload: impl Fn() + Clone + 'static,
) -> gtk4::ListBoxRow {
    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    row_box.set_margin_top(8);
    row_box.set_margin_bottom(8);
    row_box.set_margin_start(12);
    row_box.set_margin_end(12);

    let icon_gicon = ext
        .icon_key
        .as_deref()
        .map(jd_icon::resolve)
        .unwrap_or_else(|| jd_icon::resolve(crate::gui::icon_key::ICON_EXTENSION));
    let icon = gtk4::Image::from_gicon(&icon_gicon);
    icon.set_pixel_size(32);
    icon.set_valign(gtk4::Align::Center);
    row_box.append(&icon);

    let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    text_box.set_hexpand(true);
    text_box.set_valign(gtk4::Align::Center);

    let name_label = gtk4::Label::new(Some(ext.name.as_deref().unwrap_or(&ext.id)));
    name_label.set_halign(gtk4::Align::Start);
    name_label.set_xalign(0.0);
    name_label.add_css_class("heading");
    text_box.append(&name_label);

    if let Some(desc) = ext.description.as_deref().filter(|d| !d.is_empty()) {
        let desc_label = gtk4::Label::new(Some(desc));
        desc_label.set_halign(gtk4::Align::Start);
        desc_label.set_xalign(0.0);
        desc_label.set_wrap(true);
        desc_label.add_css_class("dim-label");
        text_box.append(&desc_label);
    }
    row_box.append(&text_box);

    if ext.installed {
        let switch = gtk4::Switch::new();
        switch.set_active(ext.enabled);
        switch.set_valign(gtk4::Align::Center);

        let extensions_c = extensions.clone();
        let id = ext.id.clone();
        switch.connect_state_notify(move |s| {
            let active = s.is_active();
            let e = extensions_c.clone();
            let id = id.clone();
            api_fire(move || {
                let _ = e.set_enabled(&id, active);
            });
        });
        row_box.append(&switch);
    } else {
        let install_btn = gtk4::Button::with_label(tr!("Install").as_ref());
        install_btn.set_valign(gtk4::Align::Center);

        let extensions_c = extensions.clone();
        let id = ext.id.clone();
        let reload_c = reload.clone();
        install_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label(tr!("Installing…").as_ref());
            let e = extensions_c.clone();
            let id = id.clone();
            let reload_c = reload_c.clone();
            api_call(
                move || e.install(&id),
                move |_| reload_c(),
            );
        });
        row_box.append(&install_btn);
    }

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&row_box));
    row.set_activatable(false);
    row.set_selectable(false);
    row
}
