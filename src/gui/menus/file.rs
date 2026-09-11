use adw::prelude::*;
use gtk4::gio;

use super::{action_button, action_button_with_accel, custom_item, disabled_button, submenu_button, MenuChildren};

pub fn build() -> (gio::Menu, MenuChildren) {
    // Backup submenu
    let backup_menu = gio::Menu::new();
    let backup_section = gio::Menu::new();
    backup_section.append_item(&custom_item(
        tr!("Backup all settings").as_ref(),
        None,
        "backup-save",
    ));
    backup_section.append_item(&custom_item(
        tr!("Restore settings").as_ref(),
        None,
        "backup-load",
    ));
    backup_menu.append_section(None, &backup_section);
    let backup_popover = gtk4::PopoverMenu::from_model(Some(&backup_menu));
    backup_popover.add_child(
        &disabled_button("save", tr!("Backup all settings").as_ref()),
        "backup-save",
    );
    backup_popover.add_child(
        &disabled_button("load", tr!("Restore settings").as_ref()),
        "backup-load",
    );

    // File menu
    let file_menu = gio::Menu::new();
    let file_top_section = gio::Menu::new();
    file_top_section.append_item(&custom_item(
        tr!("Analyse Text with Links").as_ref(),
        Some("win.add-links"),
        "file-analyse",
    ));
    file_top_section.append_item(&custom_item(
        tr!("Add Container").as_ref(),
        None,
        "file-container",
    ));
    file_menu.append_section(None, &file_top_section);

    let file_backup_section = gio::Menu::new();
    file_backup_section.append_item(&custom_item(
        tr!("Backup").as_ref(),
        None,
        "file-backup",
    ));
    file_menu.append_section(None, &file_backup_section);

    let file_exit_section = gio::Menu::new();
    file_exit_section.append_item(&custom_item(
        tr!("Restart").as_ref(),
        None,
        "file-restart",
    ));
    file_exit_section.append_item(&custom_item(
        tr!("Exit").as_ref(),
        Some("win.exit"),
        "file-exit",
    ));
    file_menu.append_section(None, &file_exit_section);

    let children: MenuChildren = vec![
        (
            action_button_with_accel(
                "add",
                tr!("Analyse Text with Links").as_ref(),
                "win.add-links",
                "<Primary>L",
                None,
            )
            .upcast(),
            "file-analyse".to_string(),
        ),
        (
            disabled_button("addContainer", tr!("Add Container").as_ref()).upcast(),
            "file-container".to_string(),
        ),
        (
            submenu_button("backup", tr!("Backup").as_ref(), &backup_popover).upcast(),
            "file-backup".to_string(),
        ),
        (
            disabled_button("restart", tr!("Restart").as_ref()).upcast(),
            "file-restart".to_string(),
        ),
        (
            action_button("exit", tr!("Exit").as_ref(), "win.exit").upcast(),
            "file-exit".to_string(),
        ),
    ];

    (file_menu, children)
}
