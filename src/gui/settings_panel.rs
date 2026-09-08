use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use crate::gui::components::settings::{
    AccountManagerPage, ExtensionManagerPage, GeneralSettingsPage, PackageManagerPage,
    ScriptsPage,
};
use crate::gui::jd_icon;
use crate::jd::GraphicalUserInterfaceSettings;

pub struct SettingsPanel {
    pub widget: adw::NavigationSplitView,
    /// The adw::TabPage for the settings tab when it is open.
    tab: Rc<RefCell<Option<adw::TabPage>>>,
    /// The tab that was selected before settings was opened.
    before_tab: Rc<RefCell<Option<adw::TabPage>>>,
}

impl SettingsPanel {
    pub fn build() -> Self {
        let listbox = gtk4::ListBox::new();
        listbox.set_selection_mode(gtk4::SelectionMode::Single);
        listbox.add_css_class("navigation-sidebar");
        listbox.set_margin_top(12);
        listbox.set_margin_bottom(12);
        listbox.set_margin_start(12);
        listbox.set_margin_end(12);

        let pages: Vec<adw::NavigationPage> = vec![
            adw::NavigationPage::new(&GeneralSettingsPage::build(), tr!("General").as_ref()),
            adw::NavigationPage::new(
                &AccountManagerPage::build(),
                tr!("Account Manager").as_ref(),
            ),
            adw::NavigationPage::new(
                &PackageManagerPage::build(),
                tr!("Package Manager").as_ref(),
            ),
            adw::NavigationPage::new(
                &ExtensionManagerPage::build(),
                tr!("Extension Manager").as_ref(),
            ),
            adw::NavigationPage::new(&ScriptsPage::build(), tr!("Scripts").as_ref()),
        ];
        let pages = Rc::new(pages);

        let rows = [
            Self::sidebar_row(crate::gui::icon_key::ICON_HOME, tr!("General").as_ref()),
            Self::sidebar_row(
                crate::gui::icon_key::ICON_PREMIUM,
                tr!("Account Manager").as_ref(),
            ),
            Self::sidebar_row(
                crate::gui::icon_key::ICON_PACKAGIZER,
                tr!("Package Manager").as_ref(),
            ),
            Self::sidebar_row(
                crate::gui::icon_key::ICON_EXTENSIONMANAGER,
                tr!("Extension Manager").as_ref(),
            ),
            Self::sidebar_row(crate::gui::icon_key::ICON_EVENT, tr!("Scripts").as_ref()),
        ];
        for row in &rows {
            listbox.append(row);
        }
        if let Some(first) = rows.first() {
            listbox.select_row(Some(first));
        }

        let sidebar_page = adw::NavigationPage::new(&listbox, tr!("Settings").as_ref());

        let split = adw::NavigationSplitView::new();
        split.set_sidebar_position(gtk4::PackType::Start);
        split.set_sidebar(Some(&sidebar_page));
        split.set_content(Some(&pages[0]));
        split.set_min_sidebar_width(70.0);
        split.set_max_sidebar_width(150.0);
        split.set_vexpand(true);
        split.set_hexpand(true);

        listbox.connect_row_selected({
            let split = split.clone();
            let pages = pages.clone();
            move |_, row| {
                if let Some(row) = row {
                    let index = row.index() as usize;
                    if index < pages.len() {
                        split.set_content(Some(&pages[index]));
                    }
                }
            }
        });

        Self {
            widget: split,
            tab: Rc::new(RefCell::new(None)),
            before_tab: Rc::new(RefCell::new(None)),
        }
    }

    /// Returns the current open tab page, if any.
    pub fn current_tab(&self) -> Option<adw::TabPage> {
        self.tab.borrow().clone()
    }

    /// Toggles the settings tab: opens it if closed, closes it if open.
    pub fn toggle(
        &self,
        tab_view: &adw::TabView,
        donate_tab: &adw::TabPage,
        last_tab: &Rc<RefCell<adw::TabPage>>,
        gui_settings: &GraphicalUserInterfaceSettings,
    ) {
        if let Some(tab) = self.tab.borrow().clone() {
            tab_view.close_page(&tab);
        } else {
            let g = gui_settings.clone();
            crate::gui::spawn::api_fire(move || { let _ = g.set_config_view_visible(true); });
            let pos = tab_view.page_position(donate_tab);
            let tab = tab_view.insert(&self.widget, pos);
            tab.set_title(tr!("Settings").as_ref());
            tab.set_icon(Some(&crate::gui::jd_icon::resolve(
                crate::gui::icon_key::ICON_SETTINGS,
            )));
            self.before_tab.replace(Some(last_tab.borrow().clone()));
            tab_view.set_selected_page(&tab);
            self.tab.replace(Some(tab));
        }
    }

    /// Inserts the settings tab without navigating to it (used on startup restore).
    pub fn restore(&self, tab_view: &adw::TabView, donate_tab: &adw::TabPage) {
        if self.tab.borrow().is_none() {
            let pos = tab_view.page_position(donate_tab);
            let tab = tab_view.insert(&self.widget, pos);
            tab.set_title(tr!("Settings").as_ref());
            tab.set_icon(Some(&crate::gui::jd_icon::resolve(
                crate::gui::icon_key::ICON_SETTINGS,
            )));
            self.tab.replace(Some(tab));
        }
    }

    /// Called when the settings tab page is about to be closed.
    ///
    /// Clears internal state and returns the tab that was selected before
    /// settings was opened (falls back to `default_tab`).
    pub fn on_close(&self, default_tab: &adw::TabPage) -> adw::TabPage {
        self.tab.replace(None);
        self.before_tab
            .replace(None)
            .unwrap_or_else(|| default_tab.clone())
    }

    fn sidebar_row(icon_name: &str, label: &str) -> gtk4::ListBoxRow {
        let row_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        row_box.set_halign(gtk4::Align::Center);
        row_box.set_margin_top(6);
        row_box.set_margin_bottom(6);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);

        let icon = gtk4::Image::from_gicon(&jd_icon::resolve(icon_name));
        icon.set_pixel_size(24);
        let label = gtk4::Label::new(Some(label));
        label.set_halign(gtk4::Align::Center);

        row_box.append(&icon);
        row_box.append(&label);

        let row = gtk4::ListBoxRow::new();
        row.set_child(Some(&row_box));
        row
    }
}
