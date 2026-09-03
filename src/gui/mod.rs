pub mod clipboard;
pub mod components;
pub mod spawn;
pub mod dialogs;
pub mod donate;
pub mod favicon;
pub mod downloads_panel;
pub mod fields;
pub mod jd_icon;
pub mod icon_key;
pub mod link_grabber_panel;
pub mod main_menu_bar;
pub mod main_tool_bar;
pub mod menus;
pub mod notifications;
pub mod properties_panel;
pub mod settings_panel;
pub mod tray;
pub mod remote_icon;
pub mod window;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;

use crate::app::{JdMessage, start_jd};
use crate::jd::{find_jar, GraphicalUserInterfaceSettings, JdApi, JdProcess, INTERNAL_JD_PORT};
use crate::gui::downloads_panel::DownloadsPanel;
use crate::gui::link_grabber_panel::LinkGrabberPanel;
use crate::gui::main_menu_bar::MainMenuBar;
use crate::gui::main_tool_bar::MainToolBar;
use crate::gui::settings_panel::SettingsPanel;

#[allow(deprecated)]
pub fn build_ui(app: &adw::Application) {
    // ── Window + theming ────────────────────────────────────────────────────
    window::setup_icon_theme();
    window::setup_css();

    let version = env!("CARGO_PKG_VERSION");
    let title = format!("gDownloader v{}", version);
    let is_dev = !env!("CARGO_PKG_VERSION_PRE").is_empty() || cfg!(debug_assertions);

    let window = window::create_main_window(app, &title, is_dev);
    let splash = window::create_splash(app);

    // ── JDownloader API + process ────────────────────────────────────────────
    let jar_path = find_jar();
    let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
    let process = Arc::new(Mutex::new(JdProcess::new()));

    // ── System tray ─────────────────────────────────────────────────────────
    let (tray_tx, tray_rx) = async_channel::unbounded::<tray::TrayMessage>();
    let tray_handle: Arc<Mutex<Option<ksni::blocking::Handle<tray::GDownloaderTray>>>> =
        Arc::new(Mutex::new(None));
    {
        let tray_handle = Arc::clone(&tray_handle);
        std::thread::spawn(move || {
            let service = ksni::blocking::TrayMethods::spawn_without_dbus_name(
                tray::GDownloaderTray::new(tray_tx),
            );
            match service {
                Ok(handle) => {
                    *tray_handle.lock().unwrap() = Some(handle);
                    loop {
                        std::thread::sleep(Duration::from_secs(3600));
                    }
                }
                Err(e) => log::warn!("Failed to start tray icon: {}", e),
            }
        });
    }
    {
        let window = window.clone();
        let api = Arc::clone(&api);
        glib::MainContext::default().spawn_local(async move {
            while let Ok(msg) = tray_rx.recv().await {
                match msg {
                    tray::TrayMessage::ShowWindow => window.present(),
                    tray::TrayMessage::StartDownloads => {
                        let api = api.clone();
                        std::thread::spawn(move || { let _ = api.start_all_downloads(); });
                    }
                    tray::TrayMessage::StopDownloads => {
                        let api = api.clone();
                        std::thread::spawn(move || { let _ = api.stop_all_downloads(); });
                    }
                    tray::TrayMessage::TogglePause => {
                        let api = api.clone();
                        std::thread::spawn(move || { let _ = api.toggle_pause_downloads(); });
                    }
                    tray::TrayMessage::Exit => window.close(),
                }
            }
        });
    }

    // ── Panels ───────────────────────────────────────────────────────────────
    let gui_settings = GraphicalUserInterfaceSettings::new(Arc::clone(&api));
    let tool_bar = MainToolBar::build();
    let downloads = Rc::new(DownloadsPanel::build(Arc::clone(&api), gui_settings.clone()));
    let collector = Rc::new(LinkGrabberPanel::build(Arc::clone(&api), gui_settings.clone()));
    let settings = SettingsPanel::build();

    // ── Tab view ─────────────────────────────────────────────────────────────
    let header = adw::HeaderBar::new();
    let window_title = adw::WindowTitle::new(
        title.as_str(),
        tr!("JDownloader for GNOME").as_ref(),
    );
    header.set_title_widget(Some(&window_title));

    let tab_view = adw::TabView::new();
    tab_view.set_vexpand(true);
    tab_view.set_hexpand(true);

    let download_tab = tab_view.append(&downloads.widget);
    download_tab.set_title(tr!("Download").as_ref());
    download_tab.set_icon(Some(&jd_icon::resolve(icon_key::ICON_DOWNLOAD)));

    let collector_tab = tab_view.append(&collector.widget);
    collector_tab.set_title(tr!("Link Collector").as_ref());
    collector_tab.set_icon(Some(&jd_icon::resolve(icon_key::ICON_LINKGRABBER)));

    let donate_page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let donate_tab = tab_view.append(&donate_page);
    donate_tab.set_title(tr!("Donate").as_ref());
    donate_tab.set_icon(Some(&jd_icon::resolve(icon_key::ICON_HEART)));

    let last_tab = Rc::new(RefCell::new(download_tab.clone()));
    let ignore_donate = Rc::new(Cell::new(false));

    // Link grabber context menu.
    let linkgrabber_actions = gio::SimpleActionGroup::new();
    let start_selected_action = gio::SimpleAction::new("start", None);
    start_selected_action.connect_activate({
        let api = Arc::clone(&api);
        let selection = collector.selection.clone();
        move |_, _| {
            let bitset = selection.selection();
            let ids: Vec<u64> = (0..bitset.size())
                .filter_map(|i| {
                    let pos = bitset.nth(i as u32);
                    // `selection` wraps a `TreeListModel` (package/link
                    // tree), so each item is a `TreeListRow`, not the row
                    // data directly — package rows (depth 0) have no
                    // `LinkGrabberRow`/uuid to start.
                    let tree_row = selection.item(pos).and_downcast::<gtk4::TreeListRow>()?;
                    if tree_row.depth() == 0 {
                        return None;
                    }
                    let obj = tree_row.item()?.downcast::<glib::BoxedAnyObject>().ok()?;
                    let uuid = obj.borrow::<link_grabber_panel::LinkGrabberRow>().uuid.clone();
                    uuid.parse().ok()
                })
                .collect();
            if !ids.is_empty() {
                let api = api.clone();
                std::thread::spawn(move || { let _ = api.start_linkgrabber_downloads(&ids); });
            }
        }
    });
    linkgrabber_actions.add_action(&start_selected_action);

    let start_all_action = gio::SimpleAction::new("start-all", None);
    start_all_action.connect_activate({
        let api = Arc::clone(&api);
        move |_, _| {
            let api = api.clone();
            std::thread::spawn(move || { let _ = api.start_all_downloads(); });
        }
    });
    linkgrabber_actions.add_action(&start_all_action);

    let remove_selected_action = gio::SimpleAction::new("remove", None);
    remove_selected_action.connect_activate({
        let api = Arc::clone(&api);
        let selection = collector.selection.clone();
        let child_stores = collector.child_stores.clone();
        move |_, _| {
            link_grabber_panel::remove_selected(&api, &selection, &child_stores);
        }
    });
    linkgrabber_actions.add_action(&remove_selected_action);

    collector.view.insert_action_group("linkgrabber", Some(&linkgrabber_actions));

    let context_menu = gio::Menu::new();
    context_menu.append_item(&menus::custom_item(
        tr!("Start Downloads").as_ref(),
        Some("linkgrabber.start"),
        "linkgrabber-start",
    ));
    context_menu.append_item(&menus::custom_item(
        tr!("Start All Downloads").as_ref(),
        Some("linkgrabber.start-all"),
        "linkgrabber-start-all",
    ));
    let linkgrabber_delete_section = gio::Menu::new();
    linkgrabber_delete_section.append_item(&menus::custom_item(
        tr!("Delete").as_ref(),
        Some("linkgrabber.remove"),
        "linkgrabber-remove",
    ));
    context_menu.append_section(None, &linkgrabber_delete_section);

    let context_popover = gtk4::PopoverMenu::from_model(Some(&context_menu));
    context_popover.set_parent(&collector.view);
    context_popover.add_child(
        &menus::merged_action_button(
            icon_key::ICON_MEDIA_PLAYBACK_START, 16, icon_key::ICON_ADD, 14,
            6.0, 6.0, tr!("Start Downloads").as_ref(), "linkgrabber.start", &context_popover,
        ),
        "linkgrabber-start",
    );
    context_popover.add_child(
        &menus::merged_action_button(
            icon_key::ICON_MEDIA_PLAYBACK_START, 16, icon_key::ICON_ADD, 14,
            6.0, 6.0, tr!("Start All Downloads").as_ref(), "linkgrabber.start-all",
            &context_popover,
        ),
        "linkgrabber-start-all",
    );
    context_popover.add_child(
        &menus::action_button(
            icon_key::ICON_DELETE, tr!("Delete").as_ref(), "linkgrabber.remove", &context_popover,
        ),
        "linkgrabber-remove",
    );

    let gesture = gtk4::GestureClick::new();
    gesture.set_button(gtk4::gdk::BUTTON_SECONDARY);
    gesture.connect_pressed({
        let popover = context_popover.clone();
        move |_, _, x, y| {
            popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    });
    collector.view.add_controller(gesture);

    let collector_key_controller = gtk4::EventControllerKey::new();
    collector_key_controller.connect_key_pressed({
        let api = Arc::clone(&api);
        let selection = collector.selection.clone();
        let child_stores = collector.child_stores.clone();
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Delete {
                link_grabber_panel::remove_selected(&api, &selection, &child_stores);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    collector.view.add_controller(collector_key_controller);

    // Downloads context menu.
    let downloads_actions = gio::SimpleActionGroup::new();
    let remove_downloads_action = gio::SimpleAction::new("remove", None);
    remove_downloads_action.connect_activate({
        let api = Arc::clone(&api);
        let selection = downloads.selection.clone();
        let child_stores = downloads.child_stores.clone();
        let view = downloads.view.clone();
        move |_, _| {
            downloads_panel::remove_selected(&api, &selection, &child_stores, &view);
        }
    });
    downloads_actions.add_action(&remove_downloads_action);

    downloads.view.insert_action_group("downloads", Some(&downloads_actions));

    let downloads_context_menu = gio::Menu::new();
    downloads_context_menu.append_item(&menus::custom_item(
        tr!("Delete").as_ref(),
        Some("downloads.remove"),
        "downloads-remove",
    ));

    let downloads_context_popover = gtk4::PopoverMenu::from_model(Some(&downloads_context_menu));
    downloads_context_popover.set_parent(&downloads.view);
    downloads_context_popover.add_child(
        &menus::action_button(
            icon_key::ICON_DELETE, tr!("Delete").as_ref(), "downloads.remove",
            &downloads_context_popover,
        ),
        "downloads-remove",
    );

    let downloads_gesture = gtk4::GestureClick::new();
    downloads_gesture.set_button(gtk4::gdk::BUTTON_SECONDARY);
    downloads_gesture.connect_pressed({
        let popover = downloads_context_popover.clone();
        move |_, _, x, y| {
            popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    });
    downloads.view.add_controller(downloads_gesture);

    let downloads_key_controller = gtk4::EventControllerKey::new();
    downloads_key_controller.connect_key_pressed({
        let api = Arc::clone(&api);
        let selection = downloads.selection.clone();
        let child_stores = downloads.child_stores.clone();
        let view = downloads.view.clone();
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Delete {
                downloads_panel::remove_selected(&api, &selection, &child_stores, &view);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    downloads.view.add_controller(downloads_key_controller);

    // Tab events.
    tab_view.connect_selected_page_notify({
        let window = window.clone();
        let donate_tab = donate_tab.clone();
        let last_tab = last_tab.clone();
        let ignore_donate = ignore_donate.clone();
        move |tv| {
            if let Some(page) = tv.selected_page() {
                if page == donate_tab {
                    if ignore_donate.get() {
                        return;
                    }
                    tv.set_selected_page(&*last_tab.borrow());
                    donate::show(&window);
                } else {
                    last_tab.replace(page.clone());
                }
            }
        }
    });

    tab_view.connect_close_page({
        let settings = Rc::new(settings);
        let gui_settings = gui_settings.clone();
        let download_tab = download_tab.clone();
        let ignore_donate = ignore_donate.clone();
        header.pack_start(&MainMenuBar::build(&window, {
            let settings = Rc::clone(&settings);
            let tab_view = tab_view.clone();
            let donate_tab = donate_tab.clone();
            let last_tab = last_tab.clone();
            let gui_settings = gui_settings.clone();
            Rc::new(move || {
                settings.toggle(&tab_view, &donate_tab, &last_tab, &gui_settings);
            })
        }));

        // ── Clipboard toggle init ─────────────────────────────────────────────
        let clipboard_toggle = tool_bar.clipboard_toggle.clone();
        let clipboard_initializing = Rc::new(Cell::new(true));
        clipboard_toggle.connect_toggled({
            let gui_settings = gui_settings.clone();
            let initializing = clipboard_initializing.clone();
            move |btn| {
                if initializing.get() { return; }
                let active = btn.is_active();
                let g = gui_settings.clone();
                spawn::api_fire(move || { let _ = g.set_clipboard_monitored(active); });
            }
        });

        // Toast overlay + clipboard setup.
        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_hexpand(true);
        toast_overlay.set_vexpand(true);

        clipboard::setup(
            &api,
            &clipboard_toggle,
            &window,
            app,
            &toast_overlay,
            &tab_view,
            &collector_tab,
            &tray_handle,
        );

        tool_bar.wire(
            Arc::clone(&api),
            Rc::clone(&downloads),
            Rc::clone(&collector),
            tab_view.clone(),
            download_tab.clone(),
            collector_tab.clone(),
        );

        // Refresh loops.
        downloads.start_refresh(
            Arc::clone(&api),
            window.clone(),
            app.clone(),
            toast_overlay.clone(),
        );
        collector.start_refresh(Arc::clone(&api));

        // Restore settings tab state from JD config on startup.
        {
            let (tx, rx) = async_channel::bounded::<bool>(1);
            let gui_settings_t = gui_settings.clone();
            std::thread::spawn(move || {
                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_secs(30) {
                    if gui_settings_t.is_ready() { break; }
                    std::thread::sleep(Duration::from_millis(500));
                }
                if !gui_settings_t.is_ready() { return; }
                let visible = gui_settings_t.get_config_view_visible().unwrap_or(false);
                let _ = tx.send_blocking(visible);
            });

            let settings_for_restore = Rc::clone(&settings);
            let tab_view_r = tab_view.clone();
            let donate_tab_r = donate_tab.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Ok(true) = rx.recv().await {
                    settings_for_restore.restore(&tab_view_r, &donate_tab_r);
                }
            });
        }

        // Restore clipboard monitoring state from JD config on startup.
        {
            let (tx_clip, rx_clip) = async_channel::bounded::<bool>(1);
            let gui_settings_c = gui_settings.clone();
            std::thread::spawn(move || {
                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_secs(30) {
                    if gui_settings_c.is_ready() { break; }
                    std::thread::sleep(Duration::from_millis(500));
                }
                if !gui_settings_c.is_ready() { return; }
                let active = gui_settings_c.get_clipboard_monitored().unwrap_or(true);
                let _ = tx_clip.send_blocking(active);
            });
            glib::MainContext::default().spawn_local(async move {
                if let Ok(active) = rx_clip.recv().await {
                    clipboard_toggle.set_active(active);
                    clipboard_initializing.set(false);
                }
            });
        }

        // Status bar.
        let status = gtk4::Label::new(Some(tr!("Ready").as_ref()));
        status.set_halign(gtk4::Align::Start);
        status.set_margin_start(12);
        status.set_margin_end(12);
        status.set_margin_top(6);
        status.set_margin_bottom(6);

        let status_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        status_bar.append(&status);

        let tab_bar = adw::TabBar::new();
        tab_bar.set_view(Some(&tab_view));
        tab_bar.set_expand_tabs(false);
        tab_bar.set_halign(gtk4::Align::Fill);
        tab_bar.set_hexpand(true);
        tab_bar.set_margin_start(0);
        tab_bar.set_margin_end(0);
        tab_bar.add_css_class("compact-tabbar");

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.add_top_bar(&tool_bar.widget);
        toolbar_view.add_top_bar(&tab_bar);
        toolbar_view.set_content(Some(&tab_view));
        toolbar_view.add_bottom_bar(&status_bar);

        toast_overlay.set_child(Some(&toolbar_view));
        window.set_content(Some(&toast_overlay));

        // Splash progress pulse.
        let pulsing = Rc::new(Cell::new(true));
        {
            let progress = splash.progress.clone();
            let pulsing = pulsing.clone();
            glib::source::timeout_add_local(Duration::from_millis(250), move || {
                if pulsing.get() { progress.pulse(); }
                glib::ControlFlow::Continue
            });
        }

        // JD startup message loop.
        let (tx, rx) = async_channel::bounded::<JdMessage>(32);
        glib::MainContext::default().spawn_local({
            let pulsing = pulsing.clone();
            let status = status.clone();
            let progress = splash.progress.clone();
            let main_window = window.clone();
            let splash_window = splash.window.clone();
            async move {
                let mut started = false;
                while let Ok(msg) = rx.recv().await {
                    match msg {
                        JdMessage::Started => {
                            started = true;
                            pulsing.set(false);
                            splash_window.close();
                            main_window.present();
                            status.set_label(tr!("JDownloader started").as_ref());
                        }
                        JdMessage::Error(text) => {
                            if !started {
                                pulsing.set(false);
                                splash_window.close();
                                main_window.present();
                            }
                            status.set_label(&text);
                        }
                        JdMessage::Status(text, fraction) => {
                            status.set_label(&text);
                            if fraction < 0.0 {
                                pulsing.set(true);
                            } else {
                                pulsing.set(false);
                                progress.set_fraction(fraction);
                            }
                        }
                    }
                }
            }
        });

        // Close handler.
        window::setup_close_handler(&window, Arc::clone(&api), Arc::clone(&process));

        // Launch JD.
        if let Some(path) = jar_path {
            start_jd(path, process, tx);
            splash.window.present();
        } else {
            pulsing.set(false);
            splash.window.close();
            window.present();
            status.set_label(tr!("JDownloader.jar not found. Set JDOWNLOADER_JAR.").as_ref());
        }

        // Return the close_page closure (this is still inside connect_close_page).
        move |tv, page| {
            if let Some(settings_tab) = settings.current_tab() {
                if page == &settings_tab {
                    let g = gui_settings.clone();
                    spawn::api_fire(move || { let _ = g.set_config_view_visible(false); });
                    let previous = settings.on_close(&download_tab);
                    ignore_donate.set(true);
                    tv.close_page_finish(page, true);
                    tv.set_selected_page(&previous);
                    ignore_donate.set(false);
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        }
    });
}
