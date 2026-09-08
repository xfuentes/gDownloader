#![allow(deprecated)]

use adw::prelude::*;
use gtk4::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::gui::components::{ConfigSection, ConfigSectionForm};
use crate::gui::dialogs::ScriptEditDialog;
use crate::gui::jd_icon;
use crate::gui::spawn::api_fire;
use crate::jd::{EventScripterSettings, JdApi, ScriptEntry, INTERNAL_JD_PORT};

fn wait_ready(scripts: &EventScripterSettings) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if scripts.is_ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

pub struct ScriptsPage;

impl ScriptsPage {
    pub fn build() -> gtk4::ScrolledWindow {
        let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
        let scripter = EventScripterSettings::new(Arc::clone(&api));

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
            crate::gui::icon_key::ICON_EVENT,
            tr!("Scripts").as_ref(),
            Some(
                tr!("Run JavaScript automatically when a chosen event occurs (Eventscripter).").as_ref(),
            ),
            &empty_form,
        );
        page_box.append(header_section.widget());

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

        toolbar.append(&add_btn);
        toolbar.append(&edit_btn);
        toolbar.append(&remove_btn);

        // Script table: [enabled, name, trigger]
        let store =
            gtk4::ListStore::new(&[gtk4::glib::Type::BOOL, gtk4::glib::Type::STRING, gtk4::glib::Type::STRING]);
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
        name_renderer.set_property("ellipsize", gtk4::pango::EllipsizeMode::End);
        let name_col = gtk4::TreeViewColumn::new();
        name_col.set_title(tr!("Name").as_ref());
        name_col.set_expand(true);
        name_col.pack_start(&name_renderer, true);
        name_col.add_attribute(&name_renderer, "text", 1);
        tree.append_column(&name_col);

        let trigger_renderer = gtk4::CellRendererText::new();
        trigger_renderer.set_property("ellipsize", gtk4::pango::EllipsizeMode::End);
        let trigger_col = gtk4::TreeViewColumn::new();
        trigger_col.set_title(tr!("Event trigger").as_ref());
        trigger_col.pack_start(&trigger_renderer, true);
        trigger_col.add_attribute(&trigger_renderer, "text", 2);
        tree.append_column(&trigger_col);

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
        let scripts: Rc<RefCell<Vec<ScriptEntry>>> = Rc::new(RefCell::new(Vec::new()));

        fn refresh_store(store: &gtk4::ListStore, scripts: &[ScriptEntry]) {
            store.clear();
            for s in scripts {
                let iter = store.append();
                store.set_value(&iter, 0, &s.enabled.to_value());
                let name = s.name.clone().unwrap_or_default();
                store.set_value(&iter, 1, &name.to_value());
                store.set_value(&iter, 2, &s.event_trigger.label().to_value());
            }
        }

        fn save(scripter: &EventScripterSettings, scripts: &[ScriptEntry]) {
            let scripts = scripts.to_vec();
            let s = scripter.clone();
            api_fire(move || {
                let _ = s.set_scripts(&scripts);
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
        let scripts_c = scripts.clone();
        let scripter_c = scripter.clone();
        let store_c = store.clone();
        toggle_renderer.connect_toggled(move |_, path| {
            if let Some(idx) = path.indices().first().copied().filter(|&i| i >= 0).map(|i| i as usize) {
                let mut scripts_mut = scripts_c.borrow_mut();
                if let Some(s) = scripts_mut.get_mut(idx) {
                    s.enabled = !s.enabled;
                    if let Some(iter) = store_c.iter_nth_child(None, idx as i32) {
                        store_c.set_value(&iter, 0, &s.enabled.to_value());
                    }
                }
                save(&scripter_c, &scripts_mut);
            }
        });

        // Add
        let scripts_c = scripts.clone();
        let scripter_c = scripter.clone();
        let store_c = store.clone();
        add_btn.connect_clicked(move |btn| {
            let Some(root) = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) else {
                return;
            };
            let new_entry = ScriptEntry::new(tr!("New script").as_ref());
            let scripts_for_save = scripts_c.clone();
            let scripter_for_save = scripter_c.clone();
            let store_for_save = store_c.clone();
            ScriptEditDialog::show(
                &root,
                new_entry,
                Rc::new(move |saved: ScriptEntry| {
                    scripts_for_save.borrow_mut().push(saved);
                    refresh_store(&store_for_save, &scripts_for_save.borrow());
                    save(&scripter_for_save, &scripts_for_save.borrow());
                }),
            );
        });

        // Edit (also on double-click)
        let edit_action: Rc<dyn Fn(gtk4::Window)> = {
            let scripts_c = scripts.clone();
            let scripter_c = scripter.clone();
            let store_c = store.clone();
            let tree_c = tree.clone();
            Rc::new(move |root: gtk4::Window| {
                let Some(idx) = selected_index(&tree_c) else {
                    return;
                };
                let Some(entry) = scripts_c.borrow().get(idx).cloned() else {
                    return;
                };
                let scripts_for_save = scripts_c.clone();
                let scripter_for_save = scripter_c.clone();
                let store_for_save = store_c.clone();
                ScriptEditDialog::show(
                    &root,
                    entry,
                    Rc::new(move |saved: ScriptEntry| {
                        if let Some(slot) = scripts_for_save.borrow_mut().get_mut(idx) {
                            *slot = saved;
                        }
                        refresh_store(&store_for_save, &scripts_for_save.borrow());
                        save(&scripter_for_save, &scripts_for_save.borrow());
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
        let scripts_c = scripts.clone();
        let scripter_c = scripter.clone();
        let store_c = store.clone();
        let tree_c = tree.clone();
        remove_btn.connect_clicked(move |_| {
            if let Some(idx) = selected_index(&tree_c) {
                let mut scripts_mut = scripts_c.borrow_mut();
                if idx < scripts_mut.len() {
                    scripts_mut.remove(idx);
                }
                refresh_store(&store_c, &scripts_mut);
                save(&scripter_c, &scripts_mut);
            }
        });

        // Load actual values from the internal JDownloader API.
        let (tx, rx) = async_channel::bounded::<Vec<ScriptEntry>>(1);
        let scripter_for_load = scripter.clone();
        thread::spawn(move || {
            if !wait_ready(&scripter_for_load) {
                return;
            }
            let list = scripter_for_load.get_scripts().unwrap_or_default();
            let _ = tx.try_send(list);
        });

        let scripts_c = scripts.clone();
        let store_c = store.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(list) = rx.recv().await {
                scripts_c.replace(list);
                refresh_store(&store_c, &scripts_c.borrow());
            }
        });

        scrolled
    }
}
