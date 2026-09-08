#![allow(deprecated)]

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::prelude::TreeModelExtManual;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use crate::gui::dialogs::WaitDialog;
use crate::jd::JdAccounts;
use crate::jd::plugins::hoster::{builder_for, HosterAccountBuilder};

fn wait_ready(accounts: &JdAccounts) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if accounts.is_ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

pub struct AccountAddDialog;

impl AccountAddDialog {
    pub fn show(parent: &gtk4::Window, accounts: &JdAccounts, on_added: Rc<dyn Fn()>) {
        let dialog = gtk4::Window::new();
        dialog.set_transient_for(Some(parent));
        dialog.set_modal(true);
        dialog.set_resizable(true);
        dialog.set_default_size(700, 550);
        dialog.set_title(Some(tr!("Add new Account").as_ref()));

        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        vbox.set_margin_top(18);
        vbox.set_margin_bottom(18);
        vbox.set_margin_start(18);
        vbox.set_margin_end(18);

        let choose_label = gtk4::Label::new(Some(tr!("1. Choose a Hoster").as_ref()));
        choose_label.set_halign(gtk4::Align::Start);
        vbox.append(&choose_label);
        let sep1 = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        sep1.set_margin_top(0);
        vbox.append(&sep1);

        let search = gtk4::SearchEntry::new();
        search.set_halign(gtk4::Align::Fill);
        search.set_hexpand(true);
        search.set_placeholder_text(Some(tr!("Search").as_ref()));
        vbox.append(&search);

        let store = gtk4::ListStore::new(&[gtk4::glib::Type::STRING]);

        let tree = gtk4::TreeView::new();
        tree.set_model(Some(&store));
        tree.set_headers_visible(true);

        let col = gtk4::TreeViewColumn::new();
        col.set_title(tr!("Domain").as_ref());
        let renderer = gtk4::CellRendererText::new();
        col.pack_start(&renderer, true);
        col.add_attribute(&renderer, "text", 0);
        tree.append_column(&col);

        let scroll = gtk4::ScrolledWindow::builder()
            .child(&tree)
            .vexpand(true)
            .hexpand(true)
            .build();

        vbox.append(&scroll);

        let login_label =
            gtk4::Label::new(Some(tr!("2. Enter your Login Information").as_ref()));
        login_label.set_halign(gtk4::Align::Start);
        vbox.append(&login_label);
        let sep2 = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        sep2.set_margin_top(0);
        vbox.append(&sep2);

        let name_label = gtk4::Label::new(Some(tr!("Name:").as_ref()));
        let pass_label = gtk4::Label::new(Some(tr!("Pass:").as_ref()));
        name_label.set_halign(gtk4::Align::End);
        pass_label.set_halign(gtk4::Align::End);

        let name = gtk4::Entry::new();
        name.set_placeholder_text(Some(tr!("Enter Username...").as_ref()));
        name.set_hexpand(true);

        let pass = gtk4::Entry::new();
        pass.set_visibility(false);
        pass.set_placeholder_text(Some(tr!("Enter password...").as_ref()));
        pass.set_hexpand(true);

        let login_grid = gtk4::Grid::new();
        login_grid.set_row_spacing(8);
        login_grid.set_column_spacing(12);
        login_grid.set_halign(gtk4::Align::Fill);
        login_grid.attach(&name_label, 0, 0, 1, 1);
        login_grid.attach(&name, 1, 0, 1, 1);
        login_grid.attach(&pass_label, 0, 1, 1, 1);
        login_grid.attach(&pass, 1, 1, 1, 1);

        let login_container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        login_container.set_halign(gtk4::Align::Fill);
        login_container.append(&login_grid);

        vbox.append(&login_container);

        let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());
        let save_btn = gtk4::Button::with_label(tr!("Save").as_ref());
        save_btn.set_sensitive(false);

        let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        button_size_group.add_widget(&cancel_btn);
        button_size_group.add_widget(&save_btn);

        let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        button_box.set_halign(gtk4::Align::End);
        button_box.append(&save_btn);
        button_box.append(&cancel_btn);

        vbox.append(&button_box);

        dialog.set_child(Some(&vbox));
        dialog.set_default_widget(Some(&save_btn));

        let escape_controller = gtk4::EventControllerKey::new();
        escape_controller.connect_key_pressed({
            let cancel_btn = cancel_btn.clone();
            move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    cancel_btn.activate();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        });
        dialog.add_controller(escape_controller);

        // Selection handling.
        let login_label_c = login_label.clone();
        let save_btn_c = save_btn.clone();
        let selected_hoster: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let selected_hoster_c = selected_hoster.clone();

        let current_builder: Rc<RefCell<Option<Rc<dyn HosterAccountBuilder>>>> =
            Rc::new(RefCell::new(None));
        let current_builder_c = current_builder.clone();
        let login_container_c = login_container.clone();
        let login_grid_c = login_grid.clone();

        let selection = tree.selection();
        selection.connect_changed(move |sel| {
            if let Some((model, iter)) = sel.selected() {
                let h = model.get::<glib::GString>(&iter, 0).to_string();
                if !h.is_empty() {
                    let text = tr!("2. Enter your {} Login Information").replace("{}", &h);
                    login_label_c.set_label(&text);
                    selected_hoster_c.replace(h.clone());
                    save_btn_c.set_sensitive(true);

                    if let Some(child) = login_container_c.first_child() {
                        login_container_c.remove(&child);
                    }
                    if let Some(builder) = builder_for(&h) {
                        let w = builder.widget();
                        login_container_c.append(&w);
                        current_builder_c.replace(Some(builder));
                    } else {
                        login_container_c.append(&login_grid_c);
                        current_builder_c.replace(None);
                    }
                    return;
                }
            }
            save_btn_c.set_sensitive(false);
        });

        // Load premium hosters.
        let all_hosters: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let (tx, rx) = async_channel::unbounded::<Vec<String>>();
        let accounts_for_hosters = accounts.clone();
        thread::spawn(move || {
            if !wait_ready(&accounts_for_hosters) {
                return;
            }
            if let Ok(hosters) = accounts_for_hosters.list_premium_hoster() {
                let _ = tx.try_send(hosters);
            }
        });

        let all_h = all_hosters.clone();
        let store_for_load = store.clone();
        let selection_for_load = selection.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(hosters) = rx.recv().await {
                all_h.borrow_mut().clone_from(&hosters);
                for h in hosters {
                    let iter = store_for_load.append();
                    let v = h.to_value();
                    store_for_load.set_value(&iter, 0, &v);
                }
                if let Some(iter) = store_for_load.iter_first() {
                    selection_for_load.select_iter(&iter);
                }
            }
        });

        // Search filter.
        let all_h = all_hosters.clone();
        let store_for_search = store.clone();
        let selection_for_search = selection.clone();
        search.connect_changed(move |e| {
            let q = e.text().to_string().to_lowercase();
            store_for_search.clear();
            for h in all_h.borrow().iter() {
                if h.to_lowercase().contains(&q) {
                    let iter = store_for_search.append();
                    let v = h.to_value();
                    store_for_search.set_value(&iter, 0, &v);
                }
            }
            if let Some(iter) = store_for_search.iter_first() {
                selection_for_search.select_iter(&iter);
            }
        });

        // Save action.
        let dialog_c = dialog.clone();
        let on_added_c = on_added.clone();
        let accounts = accounts.clone();
        let current_builder = current_builder.clone();
        let name_c = name.clone();
        let pass_c = pass.clone();
        save_btn.connect_clicked(move |_| {
            let hoster = selected_hoster.borrow().clone();
            if hoster.is_empty() {
                return;
            }
            let (username, password) = if let Some(builder) = current_builder.borrow().clone() {
                builder.credentials()
            } else {
                (name_c.text().to_string(), pass_c.text().to_string())
            };

            let (tx, rx) = async_channel::bounded::<bool>(1);
            let message = tr!("Adding account...");
            let wait = WaitDialog::show(&dialog_c, message.as_ref());
            let accounts = accounts.clone();
            thread::spawn(move || {
                let result = accounts
                    .add_account(&hoster, &username, &password)
                    .unwrap_or(false);
                let _ = tx.try_send(result);
            });

            let dialog = dialog_c.clone();
            let on_added = on_added_c.clone();
            glib::MainContext::default().spawn_local(async move {
                let result = rx.recv().await;
                wait.close();
                if let Ok(true) = result {
                    dialog.close();
                    on_added();
                }
            });
        });

        cancel_btn.connect_clicked({
            let dialog = dialog.clone();
            move |_| {
                dialog.close();
            }
        });

        dialog.present();
        save_btn.grab_focus();
    }
}
