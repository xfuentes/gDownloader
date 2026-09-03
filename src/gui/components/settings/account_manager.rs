use gtk4::glib;
use gtk4::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::gui::components::{AccountTable, ConfigSection, ConfigSectionForm};
use crate::gui::spawn::api_fire;
use crate::jd::{GeneralSettings, JdAccounts, JdApi, INTERNAL_JD_PORT};

pub struct AccountManagerPage;

impl AccountManagerPage {
    pub fn build() -> gtk4::ScrolledWindow {
        let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
        let general = GeneralSettings::new(Arc::clone(&api));
        let accounts = JdAccounts::new(Arc::clone(&api));

        let page_box = gtk4::Box::new(gtk4::Orientation::Vertical, 15);
        page_box.set_margin_top(15);
        page_box.set_margin_bottom(0);
        page_box.set_margin_start(15);
        page_box.set_margin_end(15);
        page_box.set_hexpand(true);
        page_box.set_vexpand(true);

        let label_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let checkbox_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let input_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        let form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let use_switch = gtk4::Switch::new();
        form.add_row(
            tr!("Use Account(s) to download").as_ref(),
            None,
            &use_switch,
        );

        let section = ConfigSection::new(
            crate::gui::icon_key::ICON_PREMIUM,
            tr!("Account Manager").as_ref(),
            Some(
                tr!("Enter and manage all your Premium/Gold/Platinum accounts.").as_ref(),
            ),
            &form,
        );
        page_box.append(section.widget());

        let table = AccountTable::build(&accounts);
        page_box.append(&table);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page_box));
        scrolled.set_vexpand(true);

        let general_save = general.clone();
        use_switch.connect_state_notify(move |s| {
            let active = s.is_active();
            let g = general_save.clone();
            api_fire(move || { let _ = g.set_use_available_accounts(active); });
        });

        let (tx, rx) = async_channel::bounded::<bool>(1);
        let general_for_load = general.clone();
        thread::spawn(move || {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_secs(60) {
                if general_for_load.is_ready() {
                    break;
                }
                thread::sleep(Duration::from_millis(500));
            }
            if !general_for_load.is_ready() {
                return;
            }
            if let Ok(value) = general_for_load.get_use_available_accounts() {
                let _ = tx.try_send(value);
            }
        });

        let use_switch_c = use_switch.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(value) = rx.recv().await {
                use_switch_c.set_active(value);
            }
        });

        scrolled
    }
}
