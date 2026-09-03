use gtk4::gio;

use super::{action_button, custom_item, disabled_button, submenu_button};

pub fn build() -> gtk4::PopoverMenu {
    // Windows submenu
    let windows_menu = gio::Menu::new();
    let windows_section = gio::Menu::new();
    windows_section.append_item(&custom_item(
        tr!("Download").as_ref(),
        None,
        "windows-downloads",
    ));
    windows_section.append_item(&custom_item(
        tr!("Link Collector").as_ref(),
        None,
        "windows-collector",
    ));
    windows_section.append_item(&custom_item(
        tr!("Settings").as_ref(),
        None,
        "windows-settings",
    ));
    windows_section.append_item(&custom_item(
        tr!("Donate").as_ref(),
        None,
        "windows-donate",
    ));
    windows_menu.append_section(None, &windows_section);

    let windows_popover = gtk4::PopoverMenu::from_model(Some(&windows_menu));
    windows_popover.add_child(
        &disabled_button("download", tr!("Download").as_ref()),
        "windows-downloads",
    );
    windows_popover.add_child(
        &disabled_button("linkgrabber", tr!("Link Collector").as_ref()),
        "windows-collector",
    );
    windows_popover.add_child(
        &action_button("settings", tr!("Settings").as_ref(), "win.settings", &windows_popover),
        "windows-settings",
    );
    windows_popover.add_child(
        &disabled_button("heart", tr!("Donate").as_ref()),
        "windows-donate",
    );

    // Tools menu
    let tools_menu = gio::Menu::new();
    let tools_section = gio::Menu::new();
    tools_section.append_item(&custom_item(
        tr!("Windows").as_ref(),
        None,
        "tools-windows",
    ));
    tools_menu.append_section(None, &tools_section);

    let tools_popover = gtk4::PopoverMenu::from_model(Some(&tools_menu));
    tools_popover.add_child(
        &submenu_button("extension", tr!("Windows").as_ref(), &windows_popover),
        "tools-windows",
    );

    tools_popover
}
