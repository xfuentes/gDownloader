#![allow(deprecated)]

use adw::prelude::*;
use gtk4::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::gui::components::{ConfigSection, ConfigSectionForm};
use crate::gui::dialogs::PackagizerRuleDialog;
use crate::gui::jd_icon;
use crate::gui::spawn::api_fire;
use crate::jd::{JdApi, PackagizerRule, PackagizerSettings, INTERNAL_JD_PORT};

fn wait_ready(packagizer: &PackagizerSettings) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if packagizer.is_ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

pub struct PackageManagerPage;

impl PackageManagerPage {
    pub fn build() -> gtk4::ScrolledWindow {
        let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
        let packagizer = PackagizerSettings::new(Arc::clone(&api));

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

        let enable_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);
        let enable_switch = gtk4::Switch::new();
        enable_form.add_row(tr!("Enable Package Manager").as_ref(), None, &enable_switch);

        let enable_section = ConfigSection::new(
            crate::gui::icon_key::ICON_PACKAGIZER,
            tr!("Package Manager").as_ref(),
            Some(
                tr!("Rules that automatically set the download settings of files based on their properties.").as_ref(),
            ),
            &enable_form,
        );
        page_box.append(enable_section.widget());

        // Toolbar
        fn icon_button(icon: &str, label: &str, size_group: &gtk4::SizeGroup) -> gtk4::Button {
            let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            bx.set_halign(gtk4::Align::Center);
            let img = gtk4::Image::from_gicon(&jd_icon::resolve(icon));
            img.set_pixel_size(16);
            bx.append(&img);
            bx.append(&gtk4::Label::new(Some(label)));
            let btn = gtk4::Button::new();
            btn.set_child(Some(&bx));
            btn.set_has_frame(false);
            size_group.add_widget(&btn);
            btn
        }

        let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        toolbar.set_halign(gtk4::Align::Start);
        let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        let add_btn = icon_button(crate::gui::icon_key::ICON_ADD, tr!("Add").as_ref(), &button_size_group);
        let edit_btn = icon_button(crate::gui::icon_key::ICON_EDIT, tr!("Edit").as_ref(), &button_size_group);
        let remove_btn =
            icon_button(crate::gui::icon_key::ICON_REMOVE, tr!("Remove").as_ref(), &button_size_group);
        let duplicate_btn = gtk4::Button::from_icon_name("edit-copy-symbolic");
        duplicate_btn.set_tooltip_text(Some(tr!("Duplicate").as_ref()));
        let up_btn = gtk4::Button::from_icon_name("go-up-symbolic");
        up_btn.set_tooltip_text(Some(tr!("Move up").as_ref()));
        let down_btn = gtk4::Button::from_icon_name("go-down-symbolic");
        down_btn.set_tooltip_text(Some(tr!("Move down").as_ref()));

        toolbar.append(&add_btn);
        toolbar.append(&edit_btn);
        toolbar.append(&remove_btn);
        toolbar.append(&duplicate_btn);
        toolbar.append(&up_btn);
        toolbar.append(&down_btn);

        // Rule table: [enabled, name]
        let store = gtk4::ListStore::new(&[gtk4::glib::Type::BOOL, gtk4::glib::Type::STRING]);
        let tree = gtk4::TreeView::new();
        tree.set_model(Some(&store));
        tree.set_headers_visible(true);

        let toggle_renderer = gtk4::CellRendererToggle::new();
        let enabled_col = gtk4::TreeViewColumn::new();
        enabled_col.set_title(tr!("Enabled").as_ref());
        enabled_col.pack_start(&toggle_renderer, false);
        enabled_col.add_attribute(&toggle_renderer, "active", 0);
        tree.append_column(&enabled_col);

        let name_renderer = gtk4::CellRendererText::new();
        name_renderer.set_property("ellipsize", &gtk4::pango::EllipsizeMode::End);
        let name_col = gtk4::TreeViewColumn::new();
        name_col.set_title(tr!("Name").as_ref());
        name_col.set_expand(true);
        name_col.pack_start(&name_renderer, true);
        name_col.add_attribute(&name_renderer, "text", 1);
        tree.append_column(&name_col);

        let selection = tree.selection();
        selection.set_mode(gtk4::SelectionMode::Single);

        let scroll = gtk4::ScrolledWindow::builder()
            .child(&tree)
            .vexpand(true)
            .hexpand(true)
            .min_content_height(260)
            .propagate_natural_height(true)
            .build();

        let table_container = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        table_container.append(&scroll);
        table_container.append(&toolbar);
        page_box.append(&table_container);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page_box));
        scrolled.set_vexpand(true);

        // State
        let rules: Rc<RefCell<Vec<PackagizerRule>>> = Rc::new(RefCell::new(Vec::new()));

        fn refresh_store(store: &gtk4::ListStore, rules: &[PackagizerRule]) {
            store.clear();
            for r in rules {
                let iter = store.append();
                store.set_value(&iter, 0, &r.enabled.to_value());
                let name = r.name.clone().unwrap_or_default();
                store.set_value(&iter, 1, &name.to_value());
            }
        }

        fn save(packagizer: &PackagizerSettings, rules: &[PackagizerRule]) {
            let mut rules = rules.to_vec();
            for (i, r) in rules.iter_mut().enumerate() {
                r.order = i as i32;
            }
            let p = packagizer.clone();
            api_fire(move || {
                let _ = p.set_rule_list(&rules);
            });
        }

        fn selected_index(tree: &gtk4::TreeView) -> Option<usize> {
            let (paths, _) = tree.selection().selected_rows();
            paths.first().and_then(|p| p.indices().first().copied()).and_then(|i| {
                if i >= 0 {
                    Some(i as usize)
                } else {
                    None
                }
            })
        }

        // Enable/disable via the toggle column.
        let rules_c = rules.clone();
        let packagizer_c = packagizer.clone();
        let store_c = store.clone();
        toggle_renderer.connect_toggled(move |_, path| {
            if let Some(idx) = path.indices().first().copied().filter(|&i| i >= 0).map(|i| i as usize) {
                let mut rules_mut = rules_c.borrow_mut();
                if let Some(r) = rules_mut.get_mut(idx) {
                    r.enabled = !r.enabled;
                    if let Some(iter) = store_c.iter_nth_child(None, idx as i32) {
                        store_c.set_value(&iter, 0, &r.enabled.to_value());
                    }
                }
                save(&packagizer_c, &rules_mut);
            }
        });

        // Add
        let rules_c = rules.clone();
        let packagizer_c = packagizer.clone();
        let store_c = store.clone();
        add_btn.connect_clicked(move |btn| {
            let Some(root) = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) else {
                return;
            };
            let new_rule = PackagizerRule::new(tr!("New rule").as_ref());
            let rules_for_save = rules_c.clone();
            let packagizer_for_save = packagizer_c.clone();
            let store_for_save = store_c.clone();
            PackagizerRuleDialog::show(
                &root,
                new_rule,
                Rc::new(move |saved: PackagizerRule| {
                    rules_for_save.borrow_mut().push(saved);
                    refresh_store(&store_for_save, &rules_for_save.borrow());
                    save(&packagizer_for_save, &rules_for_save.borrow());
                }),
            );
        });

        // Edit (also on double-click)
        let edit_action: Rc<dyn Fn(gtk4::Window)> = {
            let rules_c = rules.clone();
            let packagizer_c = packagizer.clone();
            let store_c = store.clone();
            let tree_c = tree.clone();
            Rc::new(move |root: gtk4::Window| {
                let Some(idx) = selected_index(&tree_c) else {
                    return;
                };
                let Some(rule) = rules_c.borrow().get(idx).cloned() else {
                    return;
                };
                let rules_for_save = rules_c.clone();
                let packagizer_for_save = packagizer_c.clone();
                let store_for_save = store_c.clone();
                PackagizerRuleDialog::show(
                    &root,
                    rule,
                    Rc::new(move |saved: PackagizerRule| {
                        if let Some(slot) = rules_for_save.borrow_mut().get_mut(idx) {
                            *slot = saved;
                        }
                        refresh_store(&store_for_save, &rules_for_save.borrow());
                        save(&packagizer_for_save, &rules_for_save.borrow());
                    }),
                );
            })
        };

        let edit_action_c = edit_action.clone();
        edit_btn.connect_clicked(move |btn| {
            if let Some(root) = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                edit_action_c(root);
            }
        });

        let edit_action_c2 = edit_action.clone();
        tree.connect_row_activated(move |tree, _, _| {
            if let Some(root) = tree.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                edit_action_c2(root);
            }
        });

        // Remove
        let rules_c = rules.clone();
        let packagizer_c = packagizer.clone();
        let store_c = store.clone();
        let tree_c = tree.clone();
        remove_btn.connect_clicked(move |_| {
            if let Some(idx) = selected_index(&tree_c) {
                let mut rules_mut = rules_c.borrow_mut();
                if idx < rules_mut.len() {
                    rules_mut.remove(idx);
                }
                refresh_store(&store_c, &rules_mut);
                save(&packagizer_c, &rules_mut);
            }
        });

        // Duplicate
        let rules_c = rules.clone();
        let packagizer_c = packagizer.clone();
        let store_c = store.clone();
        let tree_c = tree.clone();
        duplicate_btn.connect_clicked(move |_| {
            if let Some(idx) = selected_index(&tree_c) {
                let mut rules_mut = rules_c.borrow_mut();
                if let Some(original) = rules_mut.get(idx).cloned() {
                    let mut copy = original;
                    let base_name = copy.name.clone().unwrap_or_default();
                    copy.name = Some(format!("{} ({})", base_name, tr!("copy")));
                    copy.created = now_millis();
                    rules_mut.insert(idx + 1, copy);
                }
                refresh_store(&store_c, &rules_mut);
                save(&packagizer_c, &rules_mut);
            }
        });

        // Move up / down
        let rules_c = rules.clone();
        let packagizer_c = packagizer.clone();
        let store_c = store.clone();
        let tree_c = tree.clone();
        up_btn.connect_clicked(move |_| {
            if let Some(idx) = selected_index(&tree_c) {
                if idx > 0 {
                    let mut rules_mut = rules_c.borrow_mut();
                    rules_mut.swap(idx - 1, idx);
                    refresh_store(&store_c, &rules_mut);
                    save(&packagizer_c, &rules_mut);
                    tree_c
                        .selection()
                        .select_path(&gtk4::TreePath::from_indices(&[(idx - 1) as i32]));
                }
            }
        });

        let rules_c = rules.clone();
        let packagizer_c = packagizer.clone();
        let store_c = store.clone();
        let tree_c = tree.clone();
        down_btn.connect_clicked(move |_| {
            if let Some(idx) = selected_index(&tree_c) {
                let mut rules_mut = rules_c.borrow_mut();
                if idx + 1 < rules_mut.len() {
                    rules_mut.swap(idx, idx + 1);
                    refresh_store(&store_c, &rules_mut);
                    save(&packagizer_c, &rules_mut);
                    tree_c
                        .selection()
                        .select_path(&gtk4::TreePath::from_indices(&[(idx + 1) as i32]));
                }
            }
        });

        // Enable switch
        let packagizer_save = packagizer.clone();
        enable_switch.connect_state_notify(move |s| {
            let active = s.is_active();
            let p = packagizer_save.clone();
            api_fire(move || {
                let _ = p.set_packagizer_enabled(active);
            });
        });

        // Load actual values from the internal JDownloader API.
        let (tx, rx) = async_channel::bounded::<(bool, Vec<PackagizerRule>)>(1);
        let packagizer_for_load = packagizer.clone();
        thread::spawn(move || {
            if !wait_ready(&packagizer_for_load) {
                return;
            }
            let enabled = packagizer_for_load.get_packagizer_enabled().unwrap_or(true);
            let list = packagizer_for_load.get_rule_list().unwrap_or_default();
            let _ = tx.try_send((enabled, list));
        });

        let enable_switch_c = enable_switch.clone();
        let rules_c = rules.clone();
        let store_c = store.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok((enabled, list)) = rx.recv().await {
                enable_switch_c.set_active(enabled);
                rules_c.replace(list);
                refresh_store(&store_c, &rules_c.borrow());
            }
        });

        scrolled
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
