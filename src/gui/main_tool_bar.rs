use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use adw::prelude::*;
use gtk4::glib;

use crate::gui::downloads_panel::DownloadsPanel;
use crate::gui::link_grabber_panel::LinkGrabberPanel;
use crate::gui::spawn;
use crate::jd::{GeneralSettings, JdApi, ReconnectSettings, SilentModeSettings};

const TOOLBAR_ICON_SIZE: i32 = 24;

/// Mirrors JDownloader's `MenuManagerMainToolbar` default button set (see
/// `org.jdownloader.gui.toolbar.MenuManagerMainToolbar#createDefaultStructure`):
/// start/pause/stop, move top/up/down/bottom, clipboard/auto-reconnect/
/// premium/silent-mode toggles, reconnect, update.
pub struct MainToolBar {
    pub widget: gtk4::Box,

    start_btn: gtk4::Button,
    pause_btn: gtk4::ToggleButton,
    pause_badge: gtk4::Image,
    stop_btn: gtk4::Button,

    move_top_btn: gtk4::Button,
    move_up_btn: gtk4::Button,
    move_down_btn: gtk4::Button,
    move_bottom_btn: gtk4::Button,

    pub clipboard_toggle: gtk4::ToggleButton,
    clipboard_badge: gtk4::Image,
    auto_reconnect_toggle: gtk4::ToggleButton,
    auto_reconnect_badge: gtk4::Image,
    premium_toggle: gtk4::ToggleButton,
    premium_badge: gtk4::Image,
    silent_mode_toggle: gtk4::ToggleButton,
    silent_mode_badge: gtk4::Image,

    reconnect_btn: gtk4::Button,
    update_btn: gtk4::Button,
    update_icon: gtk4::Image,
}

/// Which package list a toolbar move action should act on — whichever tab
/// (Downloads or Link Collector) is currently active, mirroring how
/// JDownloader's own `MoveUpAction`/etc. delegate to whichever table is
/// showing (`AbstractMoveAction#onGuiMainTabSwitch`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ActivePanel {
    Downloads,
    Collector,
}

#[derive(Clone, Copy)]
enum MoveDir {
    Top,
    Up,
    Down,
    Bottom,
}

impl MainToolBar {
    pub fn build() -> Self {
        let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        toolbar.set_margin_start(12);
        toolbar.set_margin_end(12);
        toolbar.set_margin_top(12);
        toolbar.set_margin_bottom(12);
        toolbar.set_valign(gtk4::Align::Center);
        toolbar.set_size_request(-1, TOOLBAR_ICON_SIZE);

        let make_icon = |icon: &str| {
            let img = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(icon));
            img.set_pixel_size(TOOLBAR_ICON_SIZE);
            img
        };

        let add_btn = |icon: &str, tooltip: &str| -> gtk4::Button {
            let btn = gtk4::Button::new();
            btn.set_child(Some(&make_icon(icon)));
            btn.set_has_frame(false);
            btn.set_tooltip_text(Some(tooltip));
            btn.set_size_request(TOOLBAR_ICON_SIZE, TOOLBAR_ICON_SIZE);
            btn
        };

        // A toggle whose icon carries a small checkbox badge (bottom-left) —
        // the same merged-icon primitive used for JDownloader-style badged
        // menu entries (see `menus::merged_icon`) — always showing either
        // `checkbox_true` or `checkbox_false`, exactly mirroring
        // JDownloader's own toolbar toggles (`NewTheme#getCheckBoxImage`:
        // base icon + a checkbox merged into the same corner, headless
        // fallback `HeadlessCheckboxIconRef`).
        const BADGE_SIZE: i32 = 10;
        let add_toggle_with_badge = |icon: &str, tooltip: &str| -> (gtk4::ToggleButton, gtk4::Image) {
            let (fixed, badge) = crate::gui::menus::merged_icon(
                icon,
                TOOLBAR_ICON_SIZE,
                crate::gui::icon_key::ICON_CHECKBOX_FALSE,
                BADGE_SIZE,
                0.0,
                (TOOLBAR_ICON_SIZE - BADGE_SIZE + 2) as f64,
            );
            badge.set_can_target(false);
            badge.set_focusable(false);

            let btn = gtk4::ToggleButton::new();
            btn.set_child(Some(&fixed));
            btn.set_has_frame(false);
            btn.set_tooltip_text(Some(tooltip));
            btn.set_size_request(TOOLBAR_ICON_SIZE, TOOLBAR_ICON_SIZE);
            (btn, badge)
        };

        // Download controls
        let start_btn = add_btn(
            crate::gui::icon_key::ICON_MEDIA_PLAYBACK_START,
            tr!("Start downloads").as_ref(),
        );
        toolbar.append(&start_btn);

        // Also a badged toggle: JDownloader's own `PauseDownloadsAction`
        // extends the same `AbstractToolBarAction` base as the other
        // toolbar toggles and drives it with `setSelected(true/false)`
        // exactly like they do, so it renders with the same checkbox badge.
        let (pause_btn, pause_badge) = add_toggle_with_badge(
            crate::gui::icon_key::ICON_MEDIA_PLAYBACK_PAUSE,
            tr!("Pause downloads").as_ref(),
        );
        // Corrected by the first status poll in `wire()`; starting disabled
        // avoids a flash of "enabled" before that first poll lands.
        pause_btn.set_sensitive(false);
        toolbar.append(&pause_btn);

        let stop_btn = add_btn(
            crate::gui::icon_key::ICON_MEDIA_PLAYBACK_STOP,
            tr!("Stop downloads").as_ref(),
        );
        stop_btn.set_sensitive(false);
        toolbar.append(&stop_btn);

        toolbar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Move actions
        let move_top_btn = add_btn(crate::gui::icon_key::ICON_GO_TOP, tr!("Move to top").as_ref());
        toolbar.append(&move_top_btn);
        let move_up_btn = add_btn(crate::gui::icon_key::ICON_GO_UP, tr!("Move up").as_ref());
        toolbar.append(&move_up_btn);
        let move_down_btn = add_btn(crate::gui::icon_key::ICON_GO_DOWN, tr!("Move down").as_ref());
        toolbar.append(&move_down_btn);
        let move_bottom_btn = add_btn(crate::gui::icon_key::ICON_GO_BOTTOM, tr!("Move bottom").as_ref());
        toolbar.append(&move_bottom_btn);

        toolbar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Toggles
        let (clipboard_toggle, clipboard_badge) = add_toggle_with_badge(
            crate::gui::icon_key::ICON_CLIPBOARD,
            tr!("Clipboard monitoring").as_ref(),
        );
        toolbar.append(&clipboard_toggle);
        let (auto_reconnect_toggle, auto_reconnect_badge) = add_toggle_with_badge(
            crate::gui::icon_key::ICON_AUTO_RECONNECT,
            tr!("Auto reconnect").as_ref(),
        );
        // Corrected by the first status poll in `wire()`, once it knows
        // whether a reconnect plugin is actually configured.
        auto_reconnect_toggle.set_sensitive(false);
        toolbar.append(&auto_reconnect_toggle);
        let (premium_toggle, premium_badge) = add_toggle_with_badge(
            crate::gui::icon_key::ICON_PREMIUM,
            tr!("Use premium accounts").as_ref(),
        );
        toolbar.append(&premium_toggle);
        let (silent_mode_toggle, silent_mode_badge) = add_toggle_with_badge(
            crate::gui::icon_key::ICON_SILENTMODE,
            tr!("Silent mode").as_ref(),
        );
        toolbar.append(&silent_mode_toggle);

        toolbar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Misc
        let reconnect_btn = add_btn(crate::gui::icon_key::ICON_RECONNECT, tr!("Reconnect").as_ref());
        reconnect_btn.set_sensitive(false);
        toolbar.append(&reconnect_btn);

        let update_icon = make_icon(crate::gui::icon_key::ICON_UPDATE);
        let update_btn = gtk4::Button::new();
        update_btn.set_child(Some(&update_icon));
        update_btn.set_has_frame(false);
        update_btn.set_tooltip_text(Some(tr!("Check for updates").as_ref()));
        update_btn.set_size_request(TOOLBAR_ICON_SIZE, TOOLBAR_ICON_SIZE);
        toolbar.append(&update_btn);

        Self {
            widget: toolbar,
            start_btn,
            pause_btn,
            pause_badge,
            stop_btn,
            move_top_btn,
            move_up_btn,
            move_down_btn,
            move_bottom_btn,
            clipboard_toggle,
            clipboard_badge,
            auto_reconnect_toggle,
            auto_reconnect_badge,
            premium_toggle,
            premium_badge,
            silent_mode_toggle,
            silent_mode_badge,
            reconnect_btn,
            update_btn,
            update_icon,
        }
    }

    /// Attaches every action, enabled/disabled rule, and toggle state to the
    /// toolbar built by [`Self::build`]. Called once all the panels/tabs it
    /// depends on exist.
    #[allow(clippy::too_many_arguments)]
    pub fn wire(
        &self,
        api: Arc<JdApi>,
        downloads: Rc<DownloadsPanel>,
        collector: Rc<LinkGrabberPanel>,
        tab_view: adw::TabView,
        download_tab: adw::TabPage,
        collector_tab: adw::TabPage,
        tray_handle: Arc<Mutex<Option<ksni::blocking::Handle<crate::gui::tray::GDownloaderTray>>>>,
    ) {
        let general_settings = GeneralSettings::new(Arc::clone(&api));
        let reconnect_settings = ReconnectSettings::new(Arc::clone(&api));
        let silent_mode_settings = SilentModeSettings::new(Arc::clone(&api));

        // ── Start / Stop / Pause ────────────────────────────────────────────
        self.start_btn.connect_clicked({
            let api = api.clone();
            move |_| {
                let api = api.clone();
                spawn::api_fire(move || {
                    let _ = api.start_all_downloads();
                });
            }
        });
        self.stop_btn.connect_clicked({
            let api = api.clone();
            move |_| {
                let api = api.clone();
                spawn::api_fire(move || {
                    let _ = api.stop_all_downloads();
                });
            }
        });
        // `connect_clicked` (not `connect_toggled`) fires only on real user
        // activation, never from the status poll's own `set_active` calls
        // below — so no reentrancy guard is needed here.
        self.pause_btn.connect_clicked({
            let api = api.clone();
            let badge = self.pause_badge.clone();
            move |btn| {
                set_badge(&badge, btn.is_active());
                let api = api.clone();
                spawn::api_fire(move || {
                    let _ = api.toggle_pause_downloads();
                });
            }
        });

        // ── Settings toggles ─────────────────────────────────────────────────
        // Clipboard monitoring's own persistence (writing to JDownloader's
        // config) and re-check-on-enable behavior already live in
        // `clipboard.rs` / `gui/mod.rs`; this only keeps its checkbox badge
        // in sync with clicks.
        self.clipboard_toggle.connect_clicked({
            let badge = self.clipboard_badge.clone();
            move |btn| set_badge(&badge, btn.is_active())
        });
        self.auto_reconnect_toggle.connect_clicked({
            let reconnect_settings = reconnect_settings.clone();
            let badge = self.auto_reconnect_badge.clone();
            move |btn| {
                let active = btn.is_active();
                set_badge(&badge, active);
                let reconnect_settings = reconnect_settings.clone();
                spawn::api_fire(move || {
                    let _ = reconnect_settings.set_auto_reconnect_enabled(active);
                });
            }
        });
        self.premium_toggle.connect_clicked({
            let general_settings = general_settings.clone();
            let badge = self.premium_badge.clone();
            move |btn| {
                let active = btn.is_active();
                set_badge(&badge, active);
                let general_settings = general_settings.clone();
                spawn::api_fire(move || {
                    let _ = general_settings.set_use_available_accounts(active);
                });
            }
        });
        self.silent_mode_toggle.connect_clicked({
            let silent_mode_settings = silent_mode_settings.clone();
            let badge = self.silent_mode_badge.clone();
            move |btn| {
                let active = btn.is_active();
                set_badge(&badge, active);
                let silent_mode_settings = silent_mode_settings.clone();
                spawn::api_fire(move || {
                    let _ = silent_mode_settings.set_manual_enabled(active);
                });
            }
        });

        // ── Reconnect / Update ───────────────────────────────────────────────
        self.reconnect_btn.connect_clicked({
            let api = api.clone();
            move |_| {
                let api = api.clone();
                spawn::api_fire(move || {
                    let _ = api.do_reconnect();
                });
            }
        });
        self.update_btn.connect_clicked({
            let api = api.clone();
            move |_| {
                let api = api.clone();
                spawn::api_fire(move || {
                    let _ = api.run_update_check();
                });
            }
        });

        // ── Move actions ─────────────────────────────────────────────────────
        let active_panel: Rc<Cell<Option<ActivePanel>>> =
            Rc::new(Cell::new(Some(ActivePanel::Downloads)));

        let update_move_buttons: Rc<dyn Fn()> = {
            let move_top_btn = self.move_top_btn.clone();
            let move_up_btn = self.move_up_btn.clone();
            let move_down_btn = self.move_down_btn.clone();
            let move_bottom_btn = self.move_bottom_btn.clone();
            let downloads = downloads.clone();
            let collector = collector.clone();
            let active_panel = active_panel.clone();
            Rc::new(move || {
                let has_selection = match active_panel.get() {
                    Some(ActivePanel::Downloads) => !downloads.selected_package_uuids().is_empty(),
                    Some(ActivePanel::Collector) => !collector.selected_package_uuids().is_empty(),
                    None => false,
                };
                move_top_btn.set_sensitive(has_selection);
                move_up_btn.set_sensitive(has_selection);
                move_down_btn.set_sensitive(has_selection);
                move_bottom_btn.set_sensitive(has_selection);
            })
        };
        update_move_buttons();

        downloads.selection.connect_selection_changed({
            let f = update_move_buttons.clone();
            move |_, _, _| f()
        });
        collector.selection.connect_selection_changed({
            let f = update_move_buttons.clone();
            move |_, _, _| f()
        });
        tab_view.connect_selected_page_notify({
            let f = update_move_buttons.clone();
            let active_panel = active_panel.clone();
            let download_tab = download_tab.clone();
            let collector_tab = collector_tab.clone();
            move |tv| {
                if let Some(page) = tv.selected_page() {
                    active_panel.set(if page == download_tab {
                        Some(ActivePanel::Downloads)
                    } else if page == collector_tab {
                        Some(ActivePanel::Collector)
                    } else {
                        None
                    });
                    f();
                }
            }
        });

        let connect_move = |btn: &gtk4::Button, dir: MoveDir| {
            btn.connect_clicked({
                let api = api.clone();
                let downloads = downloads.clone();
                let collector = collector.clone();
                let active_panel = active_panel.clone();
                move |_| {
                    let (all, selected, target_collector) = match active_panel.get() {
                        Some(ActivePanel::Downloads) => (
                            downloads.all_package_uuids(),
                            downloads.selected_package_uuids(),
                            false,
                        ),
                        Some(ActivePanel::Collector) => (
                            collector.all_package_uuids(),
                            collector.selected_package_uuids(),
                            true,
                        ),
                        None => return,
                    };
                    let Some(after) = move_after_id(&all, &selected, dir) else {
                        return;
                    };
                    let api = api.clone();
                    spawn::api_fire(move || {
                        let result = if target_collector {
                            api.move_linkgrabber_packages(&selected, after)
                        } else {
                            api.move_download_packages(&selected, after)
                        };
                        if let Err(e) = result {
                            log::warn!("move packages failed: {}", e);
                        }
                    });
                }
            });
        };
        connect_move(&self.move_top_btn, MoveDir::Top);
        connect_move(&self.move_up_btn, MoveDir::Up);
        connect_move(&self.move_down_btn, MoveDir::Down);
        connect_move(&self.move_bottom_btn, MoveDir::Bottom);

        // ── Status poll (2s, mirrors the panels' own refresh loops) ─────────
        // Drives start/pause/stop enabled state, the four toggles' on/off
        // display (button + badge), the reconnect-availability gate, and the
        // update icon swap (`ICON_UPDATE` -> `ICON_UPDATE_B`, JDownloader's
        // own "update pending" icon).
        {
            let api = api.clone();
            let reconnect_settings = reconnect_settings.clone();
            let silent_mode_settings = silent_mode_settings.clone();
            let start_btn = self.start_btn.clone();
            let pause_btn = self.pause_btn.clone();
            let pause_badge = self.pause_badge.clone();
            let stop_btn = self.stop_btn.clone();
            let clipboard_toggle = self.clipboard_toggle.clone();
            let clipboard_badge = self.clipboard_badge.clone();
            let auto_reconnect_toggle = self.auto_reconnect_toggle.clone();
            let auto_reconnect_badge = self.auto_reconnect_badge.clone();
            let premium_toggle = self.premium_toggle.clone();
            let premium_badge = self.premium_badge.clone();
            let silent_mode_toggle = self.silent_mode_toggle.clone();
            let silent_mode_badge = self.silent_mode_badge.clone();
            let reconnect_btn = self.reconnect_btn.clone();
            let update_icon = self.update_icon.clone();
            let tray_handle = tray_handle.clone();

            glib::source::timeout_add_local(Duration::from_secs(2), move || {
                let api = api.clone();
                let reconnect_settings = reconnect_settings.clone();
                let silent_mode_settings = silent_mode_settings.clone();
                let start_btn = start_btn.clone();
                let pause_btn = pause_btn.clone();
                let pause_badge = pause_badge.clone();
                let stop_btn = stop_btn.clone();
                let clipboard_toggle = clipboard_toggle.clone();
                let clipboard_badge = clipboard_badge.clone();
                let auto_reconnect_toggle = auto_reconnect_toggle.clone();
                let auto_reconnect_badge = auto_reconnect_badge.clone();
                let premium_toggle = premium_toggle.clone();
                let premium_badge = premium_badge.clone();
                let silent_mode_toggle = silent_mode_toggle.clone();
                let silent_mode_badge = silent_mode_badge.clone();
                let reconnect_btn = reconnect_btn.clone();
                let update_icon = update_icon.clone();
                let tray_handle = tray_handle.clone();

                spawn::api_call(
                    move || {
                        let status = api.get_toolbar_status().unwrap_or_default();
                        let update_available = api.is_update_available().unwrap_or(false);
                        let reconnect_configured = reconnect_settings
                            .get_active_plugin_id()
                            .map(|id| id != crate::jd::settings::reconnect_settings::DUMMY_ROUTER_PLUGIN_ID)
                            .unwrap_or(false);
                        let silent_mode = silent_mode_settings.get_manual_enabled().unwrap_or(false);
                        (status, update_available, reconnect_configured, silent_mode)
                    },
                    move |(status, update_available, reconnect_configured, silent_mode)| {
                        start_btn.set_sensitive(!status.running);
                        pause_btn.set_sensitive(status.running || status.pause);
                        pause_btn.set_active(status.pause);
                        set_badge(&pause_badge, status.pause);
                        stop_btn.set_sensitive(status.running);

                        if let Some(handle) = tray_handle.lock().unwrap().clone() {
                            let _ = handle.update(|tray| {
                                tray.set_status(
                                    status.running,
                                    status.pause,
                                    status.clipboard_monitored,
                                    status.auto_reconnect,
                                    reconnect_configured,
                                    status.use_premium_accounts,
                                )
                            });
                        }

                        clipboard_toggle.set_active(status.clipboard_monitored);
                        set_badge(&clipboard_badge, status.clipboard_monitored);

                        auto_reconnect_toggle.set_sensitive(reconnect_configured);
                        auto_reconnect_toggle.set_active(status.auto_reconnect);
                        set_badge(&auto_reconnect_badge, status.auto_reconnect);
                        reconnect_btn.set_sensitive(reconnect_configured);

                        premium_toggle.set_active(status.use_premium_accounts);
                        set_badge(&premium_badge, status.use_premium_accounts);

                        silent_mode_toggle.set_active(silent_mode);
                        set_badge(&silent_mode_badge, silent_mode);

                        update_icon.set_from_gicon(&crate::gui::jd_icon::resolve(if update_available {
                            crate::gui::icon_key::ICON_UPDATE_B
                        } else {
                            crate::gui::icon_key::ICON_UPDATE
                        }));
                    },
                );
                glib::ControlFlow::Continue
            });
        }
    }
}

/// Swaps a toggle's checkbox badge between `checkbox_true`/`checkbox_false`
/// to reflect its current state.
fn set_badge(badge: &gtk4::Image, active: bool) {
    badge.set_from_gicon(&crate::gui::jd_icon::resolve(if active {
        crate::gui::icon_key::ICON_CHECKBOX_TRUE
    } else {
        crate::gui::icon_key::ICON_CHECKBOX_FALSE
    }));
}

/// Computes `movePackages`'s `afterDestPackageId` for moving `selected`
/// packages within the full ordered `all` package list, mirroring
/// JDownloader's own `PackageControllerTable` move actions: `Top` moves
/// before everything (id `0`); `Up`/`Down` move just past the package
/// immediately above/below the selected block; `Bottom` moves past the
/// current last package. Returns `None` when there's nothing selected.
fn move_after_id(all: &[i64], selected: &[i64], dir: MoveDir) -> Option<i64> {
    if selected.is_empty() || all.is_empty() {
        return None;
    }
    match dir {
        MoveDir::Top => Some(0),
        MoveDir::Up => {
            let first = *selected.first()?;
            let index = all.iter().position(|&u| u == first)?;
            if index >= 2 {
                Some(all[index - 2])
            } else {
                Some(0)
            }
        }
        MoveDir::Down => {
            let last = *selected.last()?;
            let index = all.iter().position(|&u| u == last)?;
            let target = (index + 1).min(all.len() - 1);
            Some(all[target])
        }
        MoveDir::Bottom => all.last().copied(),
    }
}
