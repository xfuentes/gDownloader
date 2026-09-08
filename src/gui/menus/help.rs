use adw::prelude::*;
use gtk4::gio;

use super::{custom_item, disabled_button, MenuChildren};

pub fn build() -> (gio::Menu, MenuChildren) {
    let help_menu = gio::Menu::new();
    let help_top_section = gio::Menu::new();
    help_top_section.append_item(&custom_item(
        tr!("Online Help").as_ref(),
        None,
        "help-online",
    ));
    help_top_section.append_item(&custom_item(
        tr!("Create a Log").as_ref(),
        None,
        "help-log",
    ));
    help_menu.append_section(None, &help_top_section);

    let help_updates_section = gio::Menu::new();
    help_updates_section.append_item(&custom_item(
        tr!("Find Updates").as_ref(),
        None,
        "help-updates",
    ));
    help_updates_section.append_item(&custom_item(
        tr!("Changelog").as_ref(),
        None,
        "help-changelog",
    ));
    help_menu.append_section(None, &help_updates_section);

    let help_support_section = gio::Menu::new();
    help_support_section.append_item(&custom_item(
        tr!("Contribute").as_ref(),
        None,
        "help-contribute",
    ));
    help_support_section.append_item(&custom_item(
        tr!("About JDownloader").as_ref(),
        None,
        "help-about",
    ));
    help_menu.append_section(None, &help_support_section);

    let children: MenuChildren = vec![
        (
            disabled_button("help", tr!("Online Help").as_ref()).upcast(),
            "help-online".to_string(),
        ),
        (
            disabled_button("log", tr!("Create a Log").as_ref()).upcast(),
            "help-log".to_string(),
        ),
        (
            disabled_button("update", tr!("Find Updates").as_ref()).upcast(),
            "help-updates".to_string(),
        ),
        (
            disabled_button("changelog", tr!("Changelog").as_ref()).upcast(),
            "help-changelog".to_string(),
        ),
        (
            disabled_button("heart", tr!("Contribute").as_ref()).upcast(),
            "help-contribute".to_string(),
        ),
        (
            disabled_button("about", tr!("About JDownloader").as_ref()).upcast(),
            "help-about".to_string(),
        ),
    ];

    (help_menu, children)
}
