use std::rc::Rc;

use adw::prelude::*;
use gtk4::gio;

use crate::gui::menus;

pub struct MainMenuBar;

impl MainMenuBar {
    pub fn build(window: &adw::ApplicationWindow, on_settings: Rc<dyn Fn()>) -> gtk4::Box {
        let action_group = gio::SimpleActionGroup::new();

        let settings_action = gio::SimpleAction::new("settings", None);
        settings_action.connect_activate({
            let on_settings = on_settings.clone();
            move |_, _| on_settings()
        });
        action_group.add_action(&settings_action);

        let exit_action = gio::SimpleAction::new("exit", None);
        exit_action.connect_activate({
            let window = window.clone();
            move |_, _| window.close()
        });
        action_group.add_action(&exit_action);

        window.insert_action_group("win", Some(&action_group));

        let file_popover = menus::file::build();
        let settings_popover = menus::settings::build();
        let tools_popover = menus::tools::build();
        let help_popover = menus::help::build();

        let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        bar.append(&menus::top_menu_button(tr!("File").as_ref(), &file_popover));
        bar.append(&menus::top_menu_button(
            tr!("Settings").as_ref(),
            &settings_popover,
        ));
        bar.append(&menus::top_menu_button(tr!("Tools").as_ref(), &tools_popover));
        bar.append(&menus::top_menu_button(tr!("Help").as_ref(), &help_popover));

        bar
    }
}
