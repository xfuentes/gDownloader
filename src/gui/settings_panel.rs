use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk4::glib;

use crate::gui::components::settings::{
    AccountManagerPage, ExtensionManagerPage, GeneralSettingsPage, PackageManagerPage,
    ScriptsPage,
};
use crate::gui::jd_icon;
use crate::jd::{GraphicalUserInterfaceSettings, JdApi, JdExtensions, JdProcess, INTERNAL_JD_PORT};

/// JDownloader identifiers for the EventScripter extension (`ScriptsPage`).
/// `extensions/isInstalled` takes the short optional-extension id, while
/// `extensions/isEnabled`/`setEnabled` take the extension's Java classname
/// — JDownloader itself has no single id that works for both.
const EVENTSCRIPTER_SHORT_ID: &str = "eventscripter";
const EVENTSCRIPTER_CLASSNAME: &str =
    "org.jdownloader.extensions.eventscripter.EventScripterExtension";

/// Icon size for the settings sidebar rows (General, Account Manager, ...).
const SIDEBAR_ICON_SIZE: i32 = 32;

/// Strips the pill-shaped hover/active/checked background that GTK still
/// draws on a `.flat` button — `has_frame(false)` only removes the resting
/// border, so without this the extension enable/disable badge gets an ugly
/// background as soon as it's hovered, pressed, or toggled on.
fn install_extension_toggle_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_string(
        ".extension-toggle-btn, \
         .extension-toggle-btn:hover, \
         .extension-toggle-btn:active, \
         .extension-toggle-btn:checked, \
         .extension-toggle-btn:focus { \
             background: none; \
             box-shadow: none; \
             border: none; \
             outline: none; \
             padding: 0; \
             min-width: 0; \
             min-height: 0; \
         }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub struct SettingsPanel {
    pub widget: adw::NavigationSplitView,
    /// The adw::TabPage for the settings tab when it is open.
    tab: Rc<RefCell<Option<adw::TabPage>>>,
    /// The tab that was selected before settings was opened.
    before_tab: Rc<RefCell<Option<adw::TabPage>>>,
}

impl SettingsPanel {
    pub fn build(process: Arc<Mutex<JdProcess>>, jar_path: Option<PathBuf>) -> Self {
        install_extension_toggle_css();

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
                &ExtensionManagerPage::build(process, jar_path),
                tr!("Extension Manager").as_ref(),
            ),
            adw::NavigationPage::new(&ScriptsPage::build(), tr!("Scripts").as_ref()),
        ];
        let pages = Rc::new(pages);

        let (scripts_row, scripts_icon, scripts_label, scripts_toggle, scripts_badge) =
            Self::extension_sidebar_row(crate::gui::icon_key::ICON_EVENT, tr!("Scripts").as_ref());

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
            scripts_row,
        ];
        for row in &rows {
            listbox.append(row);
        }
        if let Some(first) = rows.first() {
            listbox.select_row(Some(first));
        }

        // Wire the Scripts row's badge to the EventScripter extension's
        // enabled state, the same way JDownloader's own settings tree does:
        // unchecked/locked and the page grayed out while not installed, and
        // toggling it live-enables/disables the extension (no restart
        // needed for enable/disable, unlike install/remove).
        {
            let scripts_page = pages[4].clone();
            let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
            let extensions = JdExtensions::new(Arc::clone(&api));
            scripts_page.set_sensitive(false);

            let (tx, rx) = async_channel::bounded::<(bool, bool)>(1);
            let extensions_for_load = extensions.clone();
            thread::spawn(move || {
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(60) {
                    if api.is_ready() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(500));
                }
                let installed = extensions_for_load
                    .is_installed(EVENTSCRIPTER_SHORT_ID)
                    .unwrap_or(false);
                let enabled = installed
                    && extensions_for_load
                        .is_enabled(EVENTSCRIPTER_CLASSNAME)
                        .unwrap_or(false);
                let _ = tx.try_send((installed, enabled));
            });

            glib::MainContext::default().spawn_local(async move {
                let Ok((installed, enabled)) = rx.recv().await else {
                    return;
                };
                scripts_toggle.set_sensitive(installed);
                scripts_toggle.set_active(installed && enabled);
                Self::set_extension_row_state(
                    &scripts_icon,
                    &scripts_label,
                    &scripts_badge,
                    installed && enabled,
                );
                scripts_page.set_sensitive(installed && enabled);

                scripts_toggle.connect_toggled(move |t| {
                    let active = t.is_active();
                    Self::set_extension_row_state(&scripts_icon, &scripts_label, &scripts_badge, active);
                    scripts_page.set_sensitive(active);
                    let e = extensions.clone();
                    crate::gui::spawn::api_fire(move || {
                        let _ = e.set_enabled(EVENTSCRIPTER_CLASSNAME, active);
                    });
                });
            });
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
        icon.set_pixel_size(SIDEBAR_ICON_SIZE);
        let label = gtk4::Label::new(Some(label));
        label.set_halign(gtk4::Align::Center);

        row_box.append(&icon);
        row_box.append(&label);

        let row = gtk4::ListBoxRow::new();
        row.set_child(Some(&row_box));
        row
    }

    /// A sidebar row for a settings page backed by a JDownloader extension:
    /// a small on/off badge pinned to the icon's top-left corner lets the
    /// user enable/disable the extension without opening its page,
    /// mirroring JDownloader's own settings tree. The badge is its own
    /// frameless button sitting *on top of* (not inside) the icon, so it
    /// stays clickable to re-enable the extension even while the rest of
    /// the row is dimmed, and doesn't interfere with clicking the row
    /// itself to navigate to the page.
    fn extension_sidebar_row(
        icon_name: &str,
        label_text: &str,
    ) -> (
        gtk4::ListBoxRow,
        gtk4::Image,
        gtk4::Label,
        gtk4::ToggleButton,
        gtk4::Image,
    ) {
        const BADGE_SIZE: i32 = 14;

        // Icon + label, centered as their own group — this inner box is
        // *not* the Overlay's sizing reference; the Overlay around it is
        // stretched to the row's full width instead (confirmed via the
        // pink debug background), so the badge below actually anchors to
        // the row/tab button's own top-left corner, like in JDownloader's
        // settings tree, rather than to this centered icon+label group.
        let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        content_box.set_halign(gtk4::Align::Center);
        content_box.set_valign(gtk4::Align::Center);

        let icon = gtk4::Image::from_gicon(&jd_icon::resolve(icon_name));
        icon.set_pixel_size(SIDEBAR_ICON_SIZE);

        let label = gtk4::Label::new(Some(label_text));
        label.set_halign(gtk4::Align::Center);

        content_box.append(&icon);
        content_box.append(&label);

        let badge = gtk4::Image::from_gicon(&jd_icon::resolve(
            crate::gui::icon_key::ICON_CHECKBOX_FALSE,
        ));
        badge.set_pixel_size(BADGE_SIZE);
        badge.set_can_target(false);

        let toggle = gtk4::ToggleButton::new();
        toggle.set_child(Some(&badge));
        toggle.set_has_frame(false);
        toggle.add_css_class("flat");
        toggle.add_css_class("extension-toggle-btn");
        toggle.set_size_request(BADGE_SIZE + 4, BADGE_SIZE + 4);
        toggle.set_halign(gtk4::Align::Start);
        toggle.set_valign(gtk4::Align::Start);
        toggle.set_sensitive(false);

        let overlay = gtk4::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_margin_top(6);
        overlay.set_margin_bottom(6);
        overlay.set_margin_start(12);
        overlay.set_margin_end(12);
        overlay.set_child(Some(&content_box));
        overlay.add_overlay(&toggle);

        let row = gtk4::ListBoxRow::new();
        row.set_child(Some(&overlay));
        (row, icon, label, toggle, badge)
    }

    /// Dims an extension sidebar row's icon/label (not the toggle badge,
    /// which must stay clickable) to reflect a disabled/not-installed
    /// extension, and swaps the badge glyph itself.
    fn set_extension_row_state(icon: &gtk4::Image, label: &gtk4::Label, badge: &gtk4::Image, active: bool) {
        icon.set_opacity(if active { 1.0 } else { 0.4 });
        if active {
            label.remove_css_class("dim-label");
        } else {
            label.add_css_class("dim-label");
        }
        badge.set_from_gicon(&jd_icon::resolve(if active {
            crate::gui::icon_key::ICON_CHECKBOX_TRUE
        } else {
            crate::gui::icon_key::ICON_CHECKBOX_FALSE
        }));
    }
}
