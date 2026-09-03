use gtk4::gio;

use super::{action_button, custom_item, disabled_button};

pub fn build() -> gtk4::PopoverMenu {
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

    let settings_popover = gtk4::PopoverMenu::from_model(Some(&settings_menu));
    settings_popover.add_child(
        &action_button("settings", tr!("Settings").as_ref(), "win.settings", &settings_popover),
        "settings-settings",
    );
    settings_popover.add_child(
        &disabled_button("myjdownloader", tr!("My.JDownloader").as_ref()),
        "settings-myjd",
    );
    settings_popover.add_child(
        &disabled_button("chunks", tr!("Max. Chunks per Download").as_ref()),
        "settings-chunks",
    );
    settings_popover.add_child(
        &disabled_button("paralell", tr!("Max. simultaneous Downloads").as_ref()),
        "settings-sim",
    );
    settings_popover.add_child(
        &disabled_button("batch", tr!("Max. sim. Downloads per Hoster").as_ref()),
        "settings-host",
    );
    settings_popover.add_child(
        &disabled_button("speed", tr!("Speed Limit").as_ref()),
        "settings-speed",
    );

    settings_popover
}
