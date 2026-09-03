#![allow(deprecated)]

use adw::prelude::*;
use gtk4::glib;
use log::{error, info, warn};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::gui::dialogs::AccountAddDialog;
use crate::gui::favicon;
use crate::gui::jd_icon;
use crate::gui::spawn::api_call;
use crate::jd::{AccountQuery, AccountStorable, JdAccounts};

fn wait_ready(accounts: &JdAccounts) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if accounts.is_ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

fn account_status(acc: &AccountStorable) -> (String, gtk4::gio::Icon) {
    if !acc.enabled {
        return (tr!("Disabled").to_string(), jd_icon::resolve(crate::gui::icon_key::ICON_FALSE));
    }
    if !acc.valid {
        let text = if let Some(err) = &acc.error_string {
            err.clone()
        } else {
            tr!("Invalid").to_string()
        };
        return (text, jd_icon::resolve(crate::gui::icon_key::ICON_ERROR));
    }
    (
        tr!("Account is ok").to_string(),
        jd_icon::resolve(crate::gui::icon_key::ICON_OK),
    )
}

fn human_size(bytes: i64) -> String {
    if bytes < 0 {
        return "—".to_string();
    }
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut size = bytes as f64;
    let mut i = 0;
    while size >= 1024.0 && i + 1 < units.len() {
        size /= 1024.0;
        i += 1;
    }
    format!("{:.3} {}", size, units[i])
}

fn account_progress(acc: &AccountStorable) -> (i32, String, bool) {
    if !acc.valid || !acc.enabled {
        return (0, "—".to_string(), false);
    }
    let left = acc.traffic_left.unwrap_or(-1);
    if left < 0 {
        return (100, tr!("Unlimited").to_string(), true);
    }
    let max = acc.traffic_max.unwrap_or(0).max(0);
    let value = if max > 0 {
        ((left * 100) / max).clamp(0, 100) as i32
    } else {
        0
    };
    (value, human_size(left), true)
}

fn account_expires(ts: Option<i64>) -> String {
    match ts {
        None | Some(-1) | Some(0) => "—".to_string(),
        Some(n) => glib::DateTime::from_unix_local(n / 1000)
            .ok()
            .and_then(|dt| dt.format("%c").ok().map(|s| s.to_string()))
            .unwrap_or_else(|| "—".to_string()),
    }
}

pub struct AccountTable;

impl AccountTable {
    pub fn build(accounts: &JdAccounts) -> gtk4::Box {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        container.set_hexpand(true);
        container.set_vexpand(true);

        let view_stack = adw::ViewStack::new();
        view_stack.set_hexpand(true);
        view_stack.set_vexpand(true);
        view_stack.set_halign(gtk4::Align::Fill);
        view_stack.set_valign(gtk4::Align::Fill);

        let view_switcher = adw::ViewSwitcher::new();
        view_switcher.set_stack(Some(&view_stack));
        view_switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
        view_switcher.set_halign(gtk4::Align::Start);
        view_switcher.set_hexpand(false);

        fn icon_button(
            icon: &str,
            label: &str,
            icon_size: i32,
            size_group: &gtk4::SizeGroup,
        ) -> gtk4::Button {
            let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            bx.set_halign(gtk4::Align::Center);

            let img = gtk4::Image::from_gicon(&jd_icon::resolve(icon));
            img.set_pixel_size(icon_size);
            let lbl = gtk4::Label::new(Some(label));

            bx.append(&img);
            bx.append(&lbl);

            let btn = gtk4::Button::new();
            btn.set_child(Some(&bx));
            btn.set_has_frame(false);
            btn.set_halign(gtk4::Align::Center);
            size_group.add_widget(&btn);
            btn
        }

        let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        toolbar.set_margin_top(6);
        toolbar.set_margin_bottom(6);
        toolbar.set_margin_start(6);
        toolbar.set_margin_end(6);
        toolbar.set_halign(gtk4::Align::Start);
        toolbar.set_valign(gtk4::Align::Center);

        let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        // Icon names and sizes from JDownloader's AccountListPanel.
        let add_btn = icon_button(crate::gui::icon_key::ICON_ADD, tr!("Add").as_ref(), 20, &button_size_group);
        let remove_btn = icon_button(crate::gui::icon_key::ICON_REMOVE, tr!("Remove").as_ref(), 20, &button_size_group);
        let buy_btn = icon_button(crate::gui::icon_key::ICON_BUY, tr!("Buy").as_ref(), 20, &button_size_group);
        let refresh_btn = icon_button(crate::gui::icon_key::ICON_REFRESH, tr!("Refresh").as_ref(), 16, &button_size_group);

        buy_btn.set_sensitive(false);
        buy_btn.set_tooltip_text(Some(tr!("Buy premium account").as_ref()));

        toolbar.append(&add_btn);
        toolbar.append(&remove_btn);
        toolbar.append(&buy_btn);
        toolbar.append(&refresh_btn);

        let store = gtk4::ListStore::new(&[
            gtk4::glib::Type::STRING,
            gtk4::glib::Type::STRING,
            gtk4::glib::Type::STRING,
            gtk4::glib::Type::STRING,
            gtk4::glib::Type::STRING,
            gtk4::glib::Type::I32,
            gtk4::glib::Type::STRING,
            gtk4::glib::Type::BOOL,
            gtk4::glib::Type::BOOL,
            gtk4::gio::Icon::static_type(),
            gtk4::gio::Icon::static_type(),
        ]);

        let tree = gtk4::TreeView::new();
        tree.set_model(Some(&store));
        tree.set_headers_visible(true);
        tree.set_grid_lines(gtk4::TreeViewGridLines::Both);

        let selection = tree.selection();
        selection.set_mode(gtk4::SelectionMode::Multiple);

        let columns = [
            (tr!("Hoster"), 120),
            (tr!("Status"), 240),
            (tr!("Username"), 180),
            (tr!("Password"), 100),
            (tr!("Expire Date"), 180),
            (tr!("Download Traffic left"), 160),
        ];

        for (i, (title, min_width)) in columns.iter().enumerate() {
            let col = gtk4::TreeViewColumn::new();
            col.set_title(title.as_ref());
            if i == 0 {
                let icon = gtk4::CellRendererPixbuf::new();
                icon.set_property("icon-size", &gtk4::IconSize::Normal);
                let text = gtk4::CellRendererText::new();
                text.set_property("ellipsize", &gtk4::pango::EllipsizeMode::End);
                col.pack_start(&icon, false);
                col.pack_start(&text, true);
                col.add_attribute(&icon, "gicon", 9);
                col.add_attribute(&text, "text", 0);
            } else if i == 1 {
                let icon = gtk4::CellRendererPixbuf::new();
                icon.set_property("icon-size", &gtk4::IconSize::Normal);
                let text = gtk4::CellRendererText::new();
                col.set_sizing(gtk4::TreeViewColumnSizing::Autosize);
                col.pack_start(&icon, false);
                col.pack_start(&text, true);
                col.add_attribute(&icon, "gicon", 10);
                col.add_attribute(&text, "text", 1);
            } else if i == 5 {
                let progress = gtk4::CellRendererProgress::new();
                progress.set_property("xpad", &0u32);
                progress.set_property("ypad", &0u32);
                let text = gtk4::CellRendererText::new();
                text.set_property("ellipsize", &gtk4::pango::EllipsizeMode::End);
                col.pack_start(&text, true);
                col.pack_start(&progress, true);
                col.add_attribute(&progress, "value", 5);
                col.add_attribute(&progress, "text", 6);
                col.add_attribute(&progress, "visible", 7);
                col.add_attribute(&text, "text", 6);
                col.add_attribute(&text, "visible", 8);
            } else {
                let renderer = gtk4::CellRendererText::new();
                renderer.set_property("ellipsize", &gtk4::pango::EllipsizeMode::End);
                col.pack_start(&renderer, true);
                col.add_attribute(&renderer, "text", i as i32);
            }
            col.set_resizable(true);
            col.set_min_width(*min_width);
            tree.append_column(&col);
        }

        let scroll = gtk4::ScrolledWindow::builder()
            .child(&tree)
            .vexpand(true)
            .hexpand(true)
            .build();

        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        page.set_hexpand(true);
        page.set_vexpand(true);
        page.append(&scroll);
        page.append(&toolbar);

        let overview_page =
            view_stack.add_titled(&page, Some("overview"), tr!("Overview").as_ref());
        overview_page.set_icon_name(Some(crate::gui::icon_key::ICON_LIST));

        container.append(&view_switcher);
        container.append(&view_stack);

        let uuids: Rc<RefCell<Vec<u64>>> = Rc::new(RefCell::new(Vec::new()));
        let (tx, rx) = async_channel::unbounded::<Vec<AccountStorable>>();

        let (icon_tx, icon_rx) = async_channel::unbounded::<(String, std::path::PathBuf)>();

        let store_for_load = store.clone();
        let uuids_for_load = uuids.clone();
        let accounts_for_icons = accounts.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(list) = rx.recv().await {
                info!("account_table received {} accounts", list.len());
                store_for_load.clear();
                uuids_for_load.borrow_mut().clear();
                for acc in list {
                    uuids_for_load.borrow_mut().push(acc.uuid);
                    let iter = store_for_load.append();
                    let (progress_value, progress_text, show_progress) = account_progress(&acc);
                    let (status_text, status_icon) = account_status(&acc);
                    let values = [
                        acc.hostname.clone(),
                        status_text,
                        acc.username.clone().unwrap_or_default(),
                        "******".to_string(),
                        account_expires(acc.valid_until),
                    ];
                    for (i, v) in values.iter().enumerate() {
                        store_for_load.set_value(&iter, i as u32, &v.to_value());
                    }
                    store_for_load.set_value(&iter, 5, &(progress_value as i32).to_value());
                    store_for_load.set_value(&iter, 6, &progress_text.to_value());
                    store_for_load.set_value(&iter, 7, &show_progress.to_value());
                    store_for_load.set_value(&iter, 8, &(!show_progress).to_value());
                    let icon = jd_icon::resolve_or(&acc.hostname, "premium");
                    store_for_load.set_value(&iter, 9, &icon.to_value());
                    store_for_load.set_value(&iter, 10, &status_icon.to_value());

                    if favicon::cached(&acc.hostname).is_none() {
                        let host = acc.hostname.clone();
                        let api = Arc::clone(accounts_for_icons.api());
                        let icon_tx = icon_tx.clone();
                        thread::spawn(move || {
                            match favicon::resolve(&api, &host) {
                                Ok(path) => {
                                    let _ = icon_tx.try_send((host, path));
                                }
                                Err(e) => warn!("favicon for {}: {}", host, e),
                            }
                        });
                    }
                }
            }
        });

        let store_for_icons = store.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok((host, _)) = icon_rx.recv().await {
                let icon = jd_icon::resolve(&host);
                if let Some(mut iter) = store_for_icons.iter_first() {
                    loop {
                        if let Ok(row_host) = store_for_icons.get_value(&iter, 0).get::<String>() {
                            if row_host == host {
                                store_for_icons.set_value(&iter, 9, &icon.to_value());
                            }
                        }
                        if !store_for_icons.iter_next(&mut iter) {
                            break;
                        }
                    }
                }
            }
        });

        let load_accounts = {
            let tx = tx.clone();
            let accounts = accounts.clone();
            move || {
                let tx = tx.clone();
                let accounts = accounts.clone();
                thread::spawn(move || {
                    if !wait_ready(&accounts) {
                        return;
                    }
                    let query = AccountQuery {
                        user_name: true,
                        enabled: true,
                        valid: true,
                        valid_until: true,
                        traffic_left: true,
                        traffic_max: true,
                        error: true,
                        start_at: 0,
                        max_results: -1,
                        uuid_list: None,
                    };
                    if let Ok(list) = accounts.list_accounts(&query) {
                        let _ = tx.try_send(list);
                    }
                });
            }
        };

        let refresh: Rc<dyn Fn()> = Rc::new(load_accounts);

        // Initial load.
        refresh();

        let accounts_c = accounts.clone();
        let refresh_c = refresh.clone();
        add_btn.connect_clicked(move |btn| {
            if let Some(root) = btn.root() {
                if let Ok(window) = root.downcast::<gtk4::Window>() {
                    AccountAddDialog::show(&window, &accounts_c, refresh_c.clone());
                }
            }
        });

        let accounts_c = accounts.clone();
        let refresh_c = refresh.clone();
        let uuids_c = uuids.clone();
        let tree_c = tree.clone();
        remove_btn.connect_clicked(move |_| {
            let selection = tree_c.selection();
            let (paths, _) = selection.selected_rows();
            let ids: Vec<u64> = paths
                .iter()
                .filter_map(|p| p.indices().first().copied())
                .filter(|&i| i >= 0)
                .map(|i| i as usize)
                .filter(|&i| i < uuids_c.borrow().len())
                .map(|i| uuids_c.borrow()[i])
                .collect();

            if !ids.is_empty() {
                let acc = accounts_c.clone();
                let refresh = refresh_c.clone();
                api_call(
                    move || acc.remove_accounts(&ids),
                    move |result| match result {
                        Ok(_) => refresh(),
                        Err(e) => error!("remove_accounts failed: {}", e),
                    },
                );
            }
        });

        let accounts_c = accounts.clone();
        let refresh_c = refresh.clone();
        let uuids_c = uuids.clone();
        let tree_c = tree.clone();
        refresh_btn.connect_clicked(move |_| {
            let selection = tree_c.selection();
            let (paths, _) = selection.selected_rows();
            let mut ids: Vec<u64> = paths
                .iter()
                .filter_map(|p| p.indices().first().copied())
                .filter(|&i| i >= 0)
                .map(|i| i as usize)
                .filter(|&i| i < uuids_c.borrow().len())
                .map(|i| uuids_c.borrow()[i])
                .collect();

            if ids.is_empty() {
                ids = uuids_c.borrow().clone();
            }

            if !ids.is_empty() {
                let acc = accounts_c.clone();
                let refresh = refresh_c.clone();
                api_call(
                    move || acc.refresh_accounts(&ids, false),
                    move |_| refresh(),
                );
            }
        });

        container
    }
}
