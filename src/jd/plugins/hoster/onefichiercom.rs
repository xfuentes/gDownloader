use gtk4::prelude::*;
use std::cell::RefCell;

use super::HosterAccountBuilder;

pub struct OneFichierCom {
    container: gtk4::Box,
    api_key: gtk4::Entry,
    email: gtk4::Entry,
    password: gtk4::Entry,
    mode: RefCell<u32>,
}

impl OneFichierCom {
    pub fn new() -> Self {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        container.set_halign(gtk4::Align::Fill);
        container.set_hexpand(true);

        let type_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let type_label = gtk4::Label::new(Some(tr!("Account Type:").as_ref()));
        type_label.set_halign(gtk4::Align::Start);

        let type_strings = gtk4::StringList::new(&[
            tr!("Premium Account | API Login [Recommended]").as_ref(),
            tr!("Premium GOLD Account | API Login [Recommended]").as_ref(),
            tr!("Premium Account | Website Login").as_ref(),
            tr!("Premium GOLD Account | Website Login").as_ref(),
            tr!("Free Account with paid CDN credits").as_ref(),
            tr!("Free Account").as_ref(),
        ]);
        let type_dropdown = gtk4::DropDown::new(Some(type_strings), None::<&gtk4::Expression>);
        type_dropdown.set_selected(0);
        type_dropdown.set_halign(gtk4::Align::Fill);
        type_dropdown.set_hexpand(true);

        type_box.append(&type_label);
        type_box.append(&type_dropdown);

        let api_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        let api_label = gtk4::Label::new(Some(tr!("Premium API Key:").as_ref()));
        api_label.set_halign(gtk4::Align::Start);
        let api_key = gtk4::Entry::new();
        api_key.set_hexpand(true);
        api_key.set_visibility(false);
        api_key.set_placeholder_text(Some(tr!("Enter API key (click here to find it)").as_ref()));
        api_box.append(&api_label);
        api_box.append(&api_key);

        let web_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        let email_label = gtk4::Label::new(Some(tr!("E-Mail:").as_ref()));
        email_label.set_halign(gtk4::Align::Start);
        let email = gtk4::Entry::new();
        email.set_hexpand(true);
        email.set_placeholder_text(Some(tr!("Enter e-mail...").as_ref()));
        let password_label = gtk4::Label::new(Some(tr!("Pass:").as_ref()));
        password_label.set_halign(gtk4::Align::Start);
        let password = gtk4::Entry::new();
        password.set_visibility(false);
        password.set_hexpand(true);
        password.set_placeholder_text(Some(tr!("Enter password...").as_ref()));

        web_box.append(&email_label);
        web_box.append(&email);
        web_box.append(&password_label);
        web_box.append(&password);

        container.append(&type_box);
        container.append(&api_box);
        container.append(&web_box);

        let mode = RefCell::new(0u32);
        let api_box_c = api_box.clone();
        let web_box_c = web_box.clone();
        let mode_c = mode.clone();
        type_dropdown.connect_selected_notify(move |d| {
            let m = d.selected();
            *mode_c.borrow_mut() = m;
            let is_api = m == 0 || m == 1;
            api_box_c.set_visible(is_api);
            web_box_c.set_visible(!is_api);
        });

        api_box.set_visible(true);
        web_box.set_visible(false);

        Self {
            container,
            api_key,
            email,
            password,
            mode,
        }
    }
}

impl HosterAccountBuilder for OneFichierCom {
    fn widget(&self) -> gtk4::Box {
        self.container.clone()
    }

    fn credentials(&self) -> (String, String) {
        let m = *self.mode.borrow();
        if m == 0 || m == 1 {
            (String::new(), self.api_key.text().to_string())
        } else {
            (self.email.text().to_string(), self.password.text().to_string())
        }
    }
}
