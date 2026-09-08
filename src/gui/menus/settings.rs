use adw::prelude::*;
use gtk4::gio;

use super::{action_button, custom_item, disabled_button, MenuChildren};

pub fn build() -> (gio::Menu, MenuChildren) {
    let settings_menu = gio::Menu::new();
    let settings_top_section = gio::Menu::new();
    settings_top_section.append_item(&custom_item(
        tr!("Settings").as_ref(),
        Some("win.settings"),
        "settings-settings",
    ));
    settings_top_section.append_item(&custom_item(
        tr!("My.JDownloader").as_ref(),
        None,
        "settings-myjd",
    ));
    settings_menu.append_section(None, &settings_top_section);

    let settings_tuning_section = gio::Menu::new();
    settings_tuning_section.append_item(&custom_item(
        tr!("Max. Chunks per Download").as_ref(),
        None,
        "settings-chunks",
    ));
    settings_tuning_section.append_item(&custom_item(
        tr!("Max. simultaneous Downloads").as_ref(),
        None,
        "settings-sim",
    ));
    settings_tuning_section.append_item(&custom_item(
        tr!("Max. sim. Downloads per Hoster").as_ref(),
        None,
        "settings-host",
    ));
    settings_tuning_section.append_item(&custom_item(
        tr!("Speed Limit").as_ref(),
        None,
        "settings-speed",
    ));
    settings_menu.append_section(None, &settings_tuning_section);

    let children: MenuChildren = vec![
        (
            action_button("settings", tr!("Settings").as_ref(), "win.settings").upcast(),
            "settings-settings".to_string(),
        ),
        (
            disabled_button("myjdownloader", tr!("My.JDownloader").as_ref()).upcast(),
            "settings-myjd".to_string(),
        ),
        (
            disabled_button("chunks", tr!("Max. Chunks per Download").as_ref()).upcast(),
            "settings-chunks".to_string(),
        ),
        (
            disabled_button("paralell", tr!("Max. simultaneous Downloads").as_ref()).upcast(),
            "settings-sim".to_string(),
        ),
        (
            disabled_button("batch", tr!("Max. sim. Downloads per Hoster").as_ref()).upcast(),
            "settings-host".to_string(),
        ),
        (
            disabled_button("speed", tr!("Speed Limit").as_ref()).upcast(),
            "settings-speed".to_string(),
        ),
    ];

    (settings_menu, children)
}
