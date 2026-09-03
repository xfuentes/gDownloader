use gtk4::gio;

use super::{custom_item, disabled_button};

pub fn build() -> gtk4::PopoverMenu {
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

    let help_popover = gtk4::PopoverMenu::from_model(Some(&help_menu));
    help_popover.add_child(
        &disabled_button("help", tr!("Online Help").as_ref()),
        "help-online",
    );
    help_popover.add_child(
        &disabled_button("log", tr!("Create a Log").as_ref()),
        "help-log",
    );
    help_popover.add_child(
        &disabled_button("update", tr!("Find Updates").as_ref()),
        "help-updates",
    );
    help_popover.add_child(
        &disabled_button("changelog", tr!("Changelog").as_ref()),
        "help-changelog",
    );
    help_popover.add_child(
        &disabled_button("heart", tr!("Contribute").as_ref()),
        "help-contribute",
    );
    help_popover.add_child(
        &disabled_button("about", tr!("About JDownloader").as_ref()),
        "help-about",
    );

    help_popover
}
