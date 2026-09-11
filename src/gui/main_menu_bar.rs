use std::rc::Rc;

use adw::prelude::*;
use gtk4::gio;

use crate::gui::menus;

pub struct MainMenuBar;

impl MainMenuBar {
    pub fn build(
        window: &adw::ApplicationWindow,
        on_settings: Rc<dyn Fn()>,
        on_add_links: Rc<dyn Fn()>,
    ) -> gtk4::PopoverMenuBar {
        let action_group = gio::SimpleActionGroup::new();

        let settings_action = gio::SimpleAction::new("settings", None);
        settings_action.connect_activate({
            let on_settings = on_settings.clone();
            move |_, _| on_settings()
        });
        action_group.add_action(&settings_action);

        let add_links_action = gio::SimpleAction::new("add-links", None);
        add_links_action.connect_activate({
            let on_add_links = on_add_links.clone();
            move |_, _| on_add_links()
        });
        action_group.add_action(&add_links_action);

        let exit_action = gio::SimpleAction::new("exit", None);
        exit_action.connect_activate({
            let window = window.clone();
            move |_, _| window.close()
        });
        action_group.add_action(&exit_action);

        window.insert_action_group("win", Some(&action_group));

        let (file_menu, file_children) = menus::file::build();
        let (settings_menu, settings_children) = menus::settings::build();
        let (tools_menu, tools_children) = menus::tools::build();
        let (help_menu, help_children) = menus::help::build();

        let bar_menu = gio::Menu::new();
        bar_menu.append_submenu(Some(tr!("File").as_ref()), &file_menu);
        bar_menu.append_submenu(Some(tr!("Settings").as_ref()), &settings_menu);
        bar_menu.append_submenu(Some(tr!("Tools").as_ref()), &tools_menu);
        bar_menu.append_submenu(Some(tr!("Help").as_ref()), &help_menu);

        let bar = gtk4::PopoverMenuBar::from_model(Some(&bar_menu));
        for (widget, id) in file_children
            .into_iter()
            .chain(settings_children)
            .chain(tools_children)
            .chain(help_children)
        {
            bar.add_child(&widget, &id);
        }

        bar
    }
}
