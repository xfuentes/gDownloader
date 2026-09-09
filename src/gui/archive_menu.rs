use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;
use serde_json::Value;

use crate::gui::menus::{self, popdown_ancestor};
use crate::gui::{dialogs, icon_key};
use crate::jd::JdApi;

/// One archive found in the current selection, enough of
/// `ArchiveStatusStorable` to drive Extract Now/Abort's enabled state.
#[derive(Clone)]
struct ArchiveInfo {
    archive_id: String,
    controller_id: i64,
    controller_status: String,
}

#[derive(Default)]
struct ArchiveMenuState {
    link_ids: Vec<i64>,
    package_ids: Vec<i64>,
    archives: Vec<ArchiveInfo>,
}

impl ArchiveMenuState {
    fn abortable_controller_ids(&self) -> Vec<i64> {
        self.archives
            .iter()
            .filter(|a| {
                a.controller_id >= 0
                    && matches!(a.controller_status.as_str(), "RUNNING" | "QUEUED")
            })
            .map(|a| a.controller_id)
            .collect()
    }

    fn archive_ids(&self) -> Vec<String> {
        self.archives.iter().map(|a| a.archive_id.clone()).collect()
    }
}

/// A toggle row's widgets, bundled so its current state (needed to compute
/// the *next* state on click) travels with the badge that displays it,
/// rather than being re-derived by inspecting the icon.
#[derive(Clone)]
struct ToggleRow {
    button: gtk4::Button,
    badge: gtk4::Image,
    active: Rc<Cell<bool>>,
}

impl ToggleRow {
    fn set_active(&self, active: bool) {
        self.active.set(active);
        self.badge.set_from_gicon(&crate::gui::jd_icon::resolve(if active {
            icon_key::ICON_CHECKBOX_TRUE
        } else {
            icon_key::ICON_CHECKBOX_FALSE
        }));
    }
}

/// The "Archive(s)" context-menu submenu shown for downloads-table
/// selections, mirroring JDownloader's `ArchivesSubMenu`
/// (`org.jdownloader.extensions.extraction.contextmenu`): Extract Now,
/// Abort extraction, Validate Archive(s), Auto Extract Enabled, Set
/// Extraction Path, Set Archive Password, and a nested "Cleanup after
/// Extraction" submenu.
///
/// JDownloader determines all of this asynchronously once a selection is
/// made (`ArchiveValidator.validate`, called from a `SelectionInfoCallback`)
/// — [`ArchiveMenu::refresh`] mirrors that: it disables the whole submenu
/// immediately, then re-enables it (and fills in the toggle rows' state)
/// once `extraction/getArchiveInfo` confirms the selection actually
/// contains archives.
pub struct ArchiveMenu {
    /// Embed this in the parent context popover via `add_child`.
    pub button: gtk4::MenuButton,
    abort_btn: gtk4::Button,
    auto_extract: ToggleRow,
    cleanup_delete_files: ToggleRow,
    cleanup_delete_links: ToggleRow,
    state: Rc<RefCell<ArchiveMenuState>>,
}

/// A clickable row: `icon` + `label`, styled like `menus::action_button`
/// but driven by a plain closure instead of a `gio` action — this submenu's
/// items need live selection-dependent state that a static action name
/// can't carry.
fn row_button(icon: &str, label: &str) -> gtk4::Button {
    gtk4::Button::builder()
        .child(&menus::icon_label(icon, label))
        .has_frame(false)
        .halign(gtk4::Align::Fill)
        .build()
}

/// Like [`row_button`], but with a small checkbox badge merged into the
/// icon's corner (see `menus::merged_icon`) — the same toggle presentation
/// `main_tool_bar.rs` uses for its own toolbar toggles.
fn toggle_row_button(icon: &str, label: &str) -> ToggleRow {
    const BADGE_SIZE: i32 = 10;
    let (fixed, badge) =
        menus::merged_icon(icon, 16, icon_key::ICON_CHECKBOX_FALSE, BADGE_SIZE, 6.0, 6.0);
    badge.set_can_target(false);
    badge.set_focusable(false);
    let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    bx.set_halign(gtk4::Align::Fill);
    bx.append(&fixed);
    bx.append(&gtk4::Label::new(Some(label)));
    let button = gtk4::Button::builder()
        .child(&bx)
        .has_frame(false)
        .halign(gtk4::Align::Fill)
        .build();
    ToggleRow {
        button,
        badge,
        active: Rc::new(Cell::new(false)),
    }
}

impl ArchiveMenu {
    pub fn build(
        api: Arc<JdApi>,
        parent_window: gtk4::Window,
        toast_overlay: adw::ToastOverlay,
    ) -> Self {
        let state: Rc<RefCell<ArchiveMenuState>> = Rc::new(RefCell::new(ArchiveMenuState::default()));

        let menu = gio::Menu::new();
        menu.append_item(&menus::custom_item(
            tr!("Extract Now").as_ref(),
            None,
            "archive-extract-now",
        ));
        menu.append_item(&menus::custom_item(
            tr!("Abort extraction").as_ref(),
            None,
            "archive-abort",
        ));
        menu.append_item(&menus::custom_item(
            tr!("Validate Archive(s)").as_ref(),
            None,
            "archive-validate",
        ));
        let toggles_section = gio::Menu::new();
        toggles_section.append_item(&menus::custom_item(
            tr!("Auto Extract Enabled").as_ref(),
            None,
            "archive-auto-extract",
        ));
        toggles_section.append_item(&menus::custom_item(
            tr!("Set Extraction Path").as_ref(),
            None,
            "archive-set-path",
        ));
        toggles_section.append_item(&menus::custom_item(
            tr!("Set Archive Password").as_ref(),
            None,
            "archive-set-password",
        ));
        toggles_section.append_item(&menus::custom_item(
            tr!("Cleanup after Extraction").as_ref(),
            None,
            "archive-cleanup",
        ));
        menu.append_section(None, &toggles_section);

        let popover = gtk4::PopoverMenu::from_model(Some(&menu));

        // ── Extract Now ─────────────────────────────────────────────────
        let extract_now_btn = row_button(icon_key::ICON_RUN, tr!("Extract Now").as_ref());
        extract_now_btn.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            move |btn| {
                let s = state.borrow();
                let (link_ids, package_ids) = (s.link_ids.clone(), s.package_ids.clone());
                drop(s);
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.start_extraction_now(&link_ids, &package_ids);
                });
                popdown_ancestor(btn);
            }
        });
        popover.add_child(&extract_now_btn, "archive-extract-now");

        // ── Abort extraction ────────────────────────────────────────────
        let abort_btn = row_button(icon_key::ICON_CANCEL, tr!("Abort extraction").as_ref());
        abort_btn.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            move |btn| {
                let controller_ids = state.borrow().abortable_controller_ids();
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    for id in controller_ids {
                        let _ = api.cancel_extraction(id);
                    }
                });
                popdown_ancestor(btn);
            }
        });
        popover.add_child(&abort_btn, "archive-abort");

        // ── Validate Archive(s) ─────────────────────────────────────────
        // JDownloader opens a dedicated `DummyArchiveDialog` here, showing
        // each part's checksum/presence check interactively — not exposed
        // over the RemoteAPI at all. This instead summarizes
        // `getArchiveInfo`'s own per-file `states` as a toast, which is the
        // same completeness information (`COMPLETE`/`INCOMPLETE`/`MISSING`)
        // without needing a from-scratch validation UI.
        let validate_btn = row_button(icon_key::ICON_HASHSUM, tr!("Validate Archive(s)").as_ref());
        validate_btn.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            let toast_overlay = toast_overlay.clone();
            move |btn| {
                let s = state.borrow();
                let (link_ids, package_ids) = (s.link_ids.clone(), s.package_ids.clone());
                drop(s);
                let api = api.clone();
                let toast_overlay = toast_overlay.clone();
                crate::gui::spawn::api_call(
                    move || api.get_archive_info(&link_ids, &package_ids).unwrap_or_default(),
                    move |archives: Vec<Value>| {
                        let mut complete = 0i64;
                        let mut incomplete = 0i64;
                        let mut missing = 0i64;
                        for archive in &archives {
                            let Some(states) = archive.get("states").and_then(Value::as_object) else {
                                continue;
                            };
                            for state in states.values() {
                                match state.as_str() {
                                    Some("COMPLETE") => complete += 1,
                                    Some("INCOMPLETE") => incomplete += 1,
                                    Some("MISSING") => missing += 1,
                                    _ => {}
                                }
                            }
                        }
                        let message = if incomplete == 0 && missing == 0 {
                            tr!("Archive validated: {} file(s) present", complete).to_string()
                        } else {
                            tr!(
                                "Archive incomplete: {} present, {} incomplete, {} missing",
                                complete,
                                incomplete,
                                missing
                            )
                            .to_string()
                        };
                        toast_overlay.add_toast(adw::Toast::new(&message));
                    },
                );
                popdown_ancestor(btn);
            }
        });
        popover.add_child(&validate_btn, "archive-validate");

        // ── Auto Extract Enabled ────────────────────────────────────────
        let auto_extract = toggle_row_button(icon_key::ICON_REFRESH, tr!("Auto Extract Enabled").as_ref());
        auto_extract.button.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            let toggle = auto_extract.clone();
            move |btn| {
                let archive_ids = state.borrow().archive_ids();
                let next_active = !toggle.active.get();
                toggle.set_active(next_active);
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    for id in archive_ids {
                        let _ = api.set_archive_settings(
                            &id,
                            serde_json::json!({ "autoExtract": next_active }),
                        );
                    }
                });
                popdown_ancestor(btn);
            }
        });
        popover.add_child(&auto_extract.button, "archive-auto-extract");

        // ── Set Extraction Path ─────────────────────────────────────────
        let set_path_btn = row_button(icon_key::ICON_FOLDER, tr!("Set Extraction Path").as_ref());
        set_path_btn.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            let parent_window = parent_window.clone();
            move |btn| {
                popdown_ancestor(btn);
                let archive_ids = state.borrow().archive_ids();
                let api = api.clone();
                let file_dialog = gtk4::FileDialog::new();
                file_dialog.set_title(tr!("Set Extraction Path").as_ref());
                let parent_window = parent_window.clone();
                glib::MainContext::default().spawn_local(async move {
                    if let Ok(folder) = file_dialog.select_folder_future(Some(&parent_window)).await {
                        let Some(path) = folder.path() else {
                            return;
                        };
                        let path = path.to_string_lossy().to_string();
                        crate::gui::spawn::api_fire(move || {
                            for id in archive_ids {
                                let _ = api.set_archive_settings(
                                    &id,
                                    serde_json::json!({ "extractPath": path }),
                                );
                            }
                        });
                    }
                });
            }
        });
        popover.add_child(&set_path_btn, "archive-set-path");

        // ── Set Archive Password ────────────────────────────────────────
        let set_password_btn =
            row_button(icon_key::ICON_PASSWORD, tr!("Set Archive Password").as_ref());
        set_password_btn.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            let parent_window = parent_window.clone();
            move |btn| {
                popdown_ancestor(btn);
                let archive_ids = state.borrow().archive_ids();
                let api = api.clone();
                dialogs::ArchivePasswordDialog::show(&parent_window, move |password| {
                    let api = api.clone();
                    let archive_ids = archive_ids.clone();
                    crate::gui::spawn::api_fire(move || {
                        let _ = api.add_archive_password(&password);
                        for id in archive_ids {
                            let _ = api.set_archive_settings(
                                &id,
                                serde_json::json!({ "finalPassword": password }),
                            );
                        }
                    });
                });
            }
        });
        popover.add_child(&set_password_btn, "archive-set-password");

        // ── Cleanup after Extraction (nested submenu) ───────────────────
        let cleanup_menu = gio::Menu::new();
        cleanup_menu.append_item(&menus::custom_item(
            tr!("'Delete Archive Files' enabled").as_ref(),
            None,
            "archive-cleanup-delete-files",
        ));
        cleanup_menu.append_item(&menus::custom_item(
            tr!("'Remove Links from Downloadlist' enabled").as_ref(),
            None,
            "archive-cleanup-delete-links",
        ));
        let cleanup_popover = gtk4::PopoverMenu::from_model(Some(&cleanup_menu));

        let cleanup_delete_files = toggle_row_button(
            icon_key::ICON_DELETE,
            tr!("'Delete Archive Files' enabled").as_ref(),
        );
        cleanup_delete_files.button.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            let toggle = cleanup_delete_files.clone();
            move |btn| {
                let archive_ids = state.borrow().archive_ids();
                let next_active = !toggle.active.get();
                toggle.set_active(next_active);
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    for id in archive_ids {
                        let _ = api.set_archive_settings(
                            &id,
                            serde_json::json!({ "removeFilesAfterExtraction": next_active }),
                        );
                    }
                });
                popdown_ancestor(btn);
            }
        });
        cleanup_popover.add_child(
            &cleanup_delete_files.button,
            "archive-cleanup-delete-files",
        );

        let cleanup_delete_links = toggle_row_button(
            icon_key::ICON_LINK,
            tr!("'Remove Links from Downloadlist' enabled").as_ref(),
        );
        cleanup_delete_links.button.connect_clicked({
            let api = api.clone();
            let state = state.clone();
            let toggle = cleanup_delete_links.clone();
            move |btn| {
                let archive_ids = state.borrow().archive_ids();
                let next_active = !toggle.active.get();
                toggle.set_active(next_active);
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    for id in archive_ids {
                        let _ = api.set_archive_settings(
                            &id,
                            serde_json::json!({ "removeDownloadLinksAfterExtraction": next_active }),
                        );
                    }
                });
                popdown_ancestor(btn);
            }
        });
        cleanup_popover.add_child(
            &cleanup_delete_links.button,
            "archive-cleanup-delete-links",
        );

        let cleanup_submenu_btn = menus::submenu_button(
            icon_key::ICON_DELETE,
            tr!("Cleanup after Extraction").as_ref(),
            &cleanup_popover,
        );
        popover.add_child(&cleanup_submenu_btn, "archive-cleanup");

        let button = menus::submenu_button(icon_key::ICON_EXTRACT, tr!("Archive(s)").as_ref(), &popover);
        button.set_sensitive(false);

        Self {
            button,
            abort_btn,
            auto_extract,
            cleanup_delete_files,
            cleanup_delete_links,
            state,
        }
    }

    /// Recomputes whether `link_ids`/`package_ids` contain any archives —
    /// call right before showing the parent context menu. Disables the
    /// whole submenu until the async `getArchiveInfo`/`getArchiveSettings`
    /// round-trip comes back, mirroring JDownloader's own
    /// `ArchiveValidator` callback timing.
    pub fn refresh(&self, api: &Arc<JdApi>, link_ids: Vec<i64>, package_ids: Vec<i64>) {
        self.button.set_sensitive(false);
        self.abort_btn.set_sensitive(false);
        *self.state.borrow_mut() = ArchiveMenuState {
            link_ids: link_ids.clone(),
            package_ids: package_ids.clone(),
            archives: Vec::new(),
        };
        if link_ids.is_empty() && package_ids.is_empty() {
            return;
        }

        let api = api.clone();
        let button = self.button.clone();
        let abort_btn = self.abort_btn.clone();
        let auto_extract = self.auto_extract.clone();
        let cleanup_delete_files = self.cleanup_delete_files.clone();
        let cleanup_delete_links = self.cleanup_delete_links.clone();
        let state = self.state.clone();

        crate::gui::spawn::api_call(
            {
                let api = api.clone();
                move || api.get_archive_info(&link_ids, &package_ids).unwrap_or_default()
            },
            move |archives_json: Vec<Value>| {
                let archives: Vec<ArchiveInfo> = archives_json
                    .iter()
                    .filter_map(|a| {
                        Some(ArchiveInfo {
                            archive_id: a.get("archiveId")?.as_str()?.to_string(),
                            controller_id: a.get("controllerId").and_then(Value::as_i64).unwrap_or(-1),
                            controller_status: a
                                .get("controllerStatus")
                                .and_then(Value::as_str)
                                .unwrap_or("NA")
                                .to_string(),
                        })
                    })
                    .collect();
                if archives.is_empty() {
                    return;
                }
                button.set_sensitive(true);
                let has_abortable = archives.iter().any(|a| {
                    a.controller_id >= 0 && matches!(a.controller_status.as_str(), "RUNNING" | "QUEUED")
                });
                abort_btn.set_sensitive(has_abortable);
                let first_archive_id = archives[0].archive_id.clone();
                state.borrow_mut().archives = archives;

                crate::gui::spawn::api_call(
                    move || {
                        api.get_archive_settings(&[&first_archive_id])
                            .ok()
                            .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
                    },
                    move |settings: Option<Value>| {
                        let Some(settings) = settings else {
                            return;
                        };
                        let get = |key: &str| settings.get(key).and_then(Value::as_bool).unwrap_or(false);
                        auto_extract.set_active(get("autoExtract"));
                        cleanup_delete_files.set_active(get("removeFilesAfterExtraction"));
                        cleanup_delete_links.set_active(get("removeDownloadLinksAfterExtraction"));
                    },
                );
            },
        );
    }
}
