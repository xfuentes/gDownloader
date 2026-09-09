use adw::prelude::*;
use gtk4::glib;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::jd::{FileExistsAction, JdApi, JdDialogs, PendingDialogInfo};

/// A dialog window currently shown by [`start`], along with its own
/// answered-guard — see [`show_generic`]/[`show_file_exists`] — so a later
/// poll tick can dismiss it (see the `current`/staleness handling in
/// [`start`]) without racing whichever button the user might click at the
/// same time.
struct ShownDialog {
    window: gtk4::Window,
    answered: Rc<Cell<bool>>,
}

/// Polls JDownloader's `dialogs` RemoteAPI for dialogs it can't show
/// locally — it runs headless (no Swing/AWT display), since gDownloader
/// provides the GTK UI instead — and shows them as native GTK windows,
/// answering back through the same API. Currently the only known source is
/// EventScripter's "allow this script to run a program?" prompt, but the
/// mechanism is generic to any headless-blocked JDownloader dialog.
pub fn start(api: Arc<JdApi>, window: adw::ApplicationWindow) {
    let dialogs = JdDialogs::new(api.clone());
    // Only one dialog is ever shown at a time (JDownloader itself requires
    // dialogs to be answered oldest-first), so skip fetching a new one while
    // one is up rather than racing a second fetch against it.
    let showing = Rc::new(Cell::new(false));
    // The id/window currently shown, if any — kept so a poll tick while
    // `showing` can check whether JDownloader still lists that id at all.
    // A dialog can be resolved server-side without ever going through our
    // own answer (e.g. the download or extraction it was blocking on got
    // cancelled, or otherwise completed, while the user was still deciding)
    // — answering it afterwards fails with a 400 "Invalid ID" from
    // `dialogs/answer`, so this dismisses it locally instead of waiting for
    // that error.
    let current: Rc<RefCell<Option<(i64, ShownDialog)>>> = Rc::new(RefCell::new(None));

    glib::source::timeout_add_local(Duration::from_millis(1500), move || {
        if showing.get() {
            let dialogs_fetch = dialogs.clone();
            let (tx, rx) = async_channel::bounded::<Vec<i64>>(1);
            std::thread::spawn(move || {
                let _ = tx.try_send(dialogs_fetch.list().unwrap_or_default());
            });

            let showing_c = showing.clone();
            let current_c = current.clone();
            glib::MainContext::default().spawn_local(async move {
                let Ok(ids) = rx.recv().await else {
                    return;
                };
                if !showing_c.get() {
                    // The user already answered it themselves while this
                    // `list()` fetch was in flight — nothing to dismiss.
                    return;
                }
                let stale = current_c
                    .borrow()
                    .as_ref()
                    .is_some_and(|(id, _)| !ids.contains(id));
                if stale {
                    if let Some((id, shown)) = current_c.borrow_mut().take() {
                        log::info!(
                            "JDownloader dialog {} resolved itself before it was answered; dismissing",
                            id
                        );
                        // Marked answered *before* closing so the
                        // close-request handling in show_generic/
                        // show_file_exists doesn't also try to answer an id
                        // JDownloader has already forgotten about.
                        shown.answered.set(true);
                        shown.window.close();
                    }
                    showing_c.set(false);
                }
            });
            return glib::ControlFlow::Continue;
        }

        let (tx, rx) =
            async_channel::bounded::<Option<(i64, PendingDialogInfo, FileExistsDetails)>>(1);
        let dialogs_fetch = dialogs.clone();
        let api_fetch = api.clone();
        std::thread::spawn(move || {
            let next = dialogs_fetch
                .list()
                .ok()
                .and_then(|ids| ids.into_iter().next())
                .and_then(|id| dialogs_fetch.get(id).ok().map(|info| (id, info)));
            let next = next.map(|(id, info)| {
                let details = if info.is_file_exists() {
                    FileExistsDetails::lookup(&api_fetch, &info)
                } else {
                    FileExistsDetails::default()
                };
                (id, info, details)
            });
            let _ = tx.try_send(next);
        });

        let showing_c = showing.clone();
        let current_c = current.clone();
        let dialogs_c = dialogs.clone();
        let window_c = window.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(Some((id, info, details))) = rx.recv().await {
                showing_c.set(true);
                let shown = if info.is_file_exists() {
                    show_file_exists(&window_c, id, &info, details, dialogs_c, showing_c)
                } else {
                    show_generic(&window_c, id, &info, dialogs_c, showing_c)
                };
                *current_c.borrow_mut() = Some((id, shown));
            }
        });

        glib::ControlFlow::Continue
    });
}

/// Extra context for a [`PendingDialogInfo::is_file_exists`] dialog, gathered
/// locally instead of from the dialog's own properties — JDownloader's
/// `IfFileExistsDialogInterface` only exposes `filepath`/`packagename`/
/// `packageid`/`host` over the RemoteAPI (verified against its SVN source:
/// the real Swing dialog computes sizes/archive name itself, client-side,
/// and never serializes them, and its rename text field isn't part of the
/// interface at all — `IfFileExistsDialog.getNewName()` only exists on the
/// concrete Swing class, so a remote answer's `action: AUTO_RENAME` always
/// gets JDownloader's own computed name regardless of what's sent).
/// gDownloader can still reconstruct all of this, since it always runs
/// alongside a local JDownloader instance sharing the same filesystem and
/// the same download queue.
#[derive(Debug, Clone, Default)]
struct FileExistsDetails {
    /// Total size of the incoming download, matched against
    /// `downloadsV2/queryLinks` by filename + package name. Only resolves
    /// for a download conflict — an extraction conflict's `filepath` is a
    /// member inside the archive, not a queued download's own destination.
    incoming: Option<i64>,
    /// Byte size of the file already on disk at `filepath` (or its `.part`
    /// twin), mirroring `IfFileExistsDialog`'s own fallback.
    existing: Option<u64>,
    /// The archive's own filename, for an extraction conflict: the first
    /// download in the same package (an archive's parts are downloaded like
    /// any other link, so its filename is already sitting in the queue).
    archive_name: Option<String>,
    /// Suggested free filename for `AUTO_RENAME`, computed with the exact
    /// same algorithm as `IfFileExistsDialog`'s constructor: append `_1`,
    /// `_2`, ... before the extension until a name that doesn't exist next
    /// to `filepath` is found.
    suggested_name: String,
}

impl FileExistsDetails {
    fn lookup(api: &JdApi, info: &PendingDialogInfo) -> Self {
        let links = api
            .query_downloads()
            .map(|(_, links)| links)
            .unwrap_or_default();
        Self {
            incoming: Self::incoming_size(&links, info),
            existing: Self::existing_size(info.file_path()),
            archive_name: Self::archive_name(&links, info),
            suggested_name: suggest_new_name(info.file_path()),
        }
    }

    fn existing_size(path: &str) -> Option<u64> {
        if path.is_empty() {
            return None;
        }
        std::fs::metadata(path)
            .or_else(|_| std::fs::metadata(format!("{path}.part")))
            .ok()
            .map(|m| m.len())
    }

    fn incoming_size(links: &[Value], info: &PendingDialogInfo) -> Option<i64> {
        let file_name = std::path::Path::new(info.file_path()).file_name()?.to_str()?;
        links
            .iter()
            .find(|link| {
                link.get("name").and_then(Value::as_str) == Some(file_name)
                    && link.get("packageName").and_then(Value::as_str)
                        == Some(info.package_name())
            })
            .and_then(|link| link.get("bytesTotal").and_then(Value::as_i64))
    }

    fn archive_name(links: &[Value], info: &PendingDialogInfo) -> Option<String> {
        if info.package_name().is_empty() {
            return None;
        }
        links
            .iter()
            .find(|link| {
                link.get("packageName").and_then(Value::as_str) == Some(info.package_name())
            })
            .and_then(|link| link.get("name").and_then(Value::as_str))
            .map(str::to_string)
    }
}

/// Suggests a free filename for `AUTO_RENAME`, given the conflicting file's
/// full path. Mirrors `IfFileExistsDialog`'s constructor exactly
/// (`org.jdownloader.extensions.extraction.gui.iffileexistsdialog.
/// IfFileExistsDialog`): splits the name at its last `.`, then appends
/// `_1`, `_2`, ... before the extension until a candidate that doesn't
/// exist next to it is found.
fn suggest_new_name(path: &str) -> String {
    let path = std::path::Path::new(path);
    let dir = path.parent();
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let Some(file_name) = file_name else {
        return String::new();
    };
    let (stem, ext) = match path.extension().map(|e| e.to_string_lossy().into_owned()) {
        Some(ext) => (file_name[..file_name.len() - ext.len() - 1].to_string(), Some(ext)),
        None => (file_name, None),
    };
    let mut i = 1;
    loop {
        let candidate = match &ext {
            Some(ext) => format!("{stem}_{i}.{ext}"),
            None => format!("{stem}_{i}"),
        };
        let exists = match dir {
            Some(dir) => dir.join(&candidate).exists(),
            None => std::path::Path::new(&candidate).exists(),
        };
        if !exists {
            return candidate;
        }
        i += 1;
    }
}

/// Adds a bottom-left "Ns" countdown badge to `button_row`, mirroring
/// JDownloader's own countdown display on dialogs that have one (see the
/// download-conflict `IfFileExistsDialog`'s `STYLE_SHOW_DO_NOT_DISPLAY_AGAIN
/// | LOGIC_COUNTDOWN`). Shown unconditionally here, not just for that one
/// dialog: JDownloader silently abandons *any* headless-blocked dialog
/// without its own countdown after a short SilentMode background timeout
/// (`RemoteAPIIOHandlerWrapper.showModeless`'s fallback, ~10-30s depending
/// on the instance's `SilentModeSettings` — confirmed against the SVN and
/// against a real "Invalid ID" failure this caused), so warning the user
/// before that happens matters even where JDownloader itself doesn't.
///
/// Purely visual — reaching zero doesn't do anything on its own; the
/// polling loop in [`start`] is what actually detects and dismisses a
/// dialog JDownloader has expired server-side (see [`ShownDialog`]).
/// Ticking stops via `answered` as soon as the dialog is genuinely
/// answered or dismissed, however that happens.
fn add_countdown_badge(button_row: &gtk4::Box, answered: &Rc<Cell<bool>>) {
    const SECONDS: i32 = 60;

    let badge = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    badge.set_valign(gtk4::Align::Center);
    let icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
        crate::gui::icon_key::ICON_CANCEL,
    ));
    icon.set_pixel_size(14);
    badge.append(&icon);
    let label = gtk4::Label::new(Some(&format!("{SECONDS}s")));
    badge.append(&label);
    button_row.prepend(&badge);

    let remaining = Rc::new(Cell::new(SECONDS));
    glib::source::timeout_add_local(Duration::from_secs(1), {
        let answered = answered.clone();
        move || {
            if answered.get() {
                return glib::ControlFlow::Break;
            }
            let next = remaining.get() - 1;
            if next <= 0 {
                label.set_text("0s");
                return glib::ControlFlow::Break;
            }
            remaining.set(next);
            label.set_text(&format!("{next}s"));
            glib::ControlFlow::Continue
        }
    });
}

/// Shows the generic Allow/Deny prompt used for every dialog type except
/// [`PendingDialogInfo::is_file_exists`] (see [`show_file_exists`]).
fn show_generic(
    parent: &adw::ApplicationWindow,
    id: i64,
    info: &PendingDialogInfo,
    dialogs: JdDialogs,
    showing: Rc<Cell<bool>>,
) -> ShownDialog {
    let title = if info.title().is_empty() {
        tr!("JDownloader").to_string()
    } else {
        info.title().to_string()
    };

    let dialog = gtk4::Window::new();
    dialog.set_transient_for(Some(parent));
    dialog.set_modal(true);
    dialog.set_resizable(false);
    dialog.set_title(Some(&title));

    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(12);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
        crate::gui::icon_key::ICON_QUESTION,
    ));
    icon.set_pixel_size(48);
    icon.set_valign(gtk4::Align::Start);
    content.append(&icon);

    let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    text_box.set_hexpand(true);

    let heading_label = gtk4::Label::new(Some(&title));
    heading_label.set_halign(gtk4::Align::Start);
    heading_label.set_xalign(0.0);
    heading_label.add_css_class("heading");
    heading_label.set_wrap(true);
    heading_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    text_box.append(&heading_label);

    let message_label = gtk4::Label::new(Some(info.message()));
    message_label.set_halign(gtk4::Align::Start);
    message_label.set_xalign(0.0);
    message_label.set_wrap(true);
    message_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    text_box.append(&message_label);

    content.append(&text_box);
    outer.append(&content);

    let button_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    button_row.set_margin_top(6);
    button_row.set_margin_bottom(12);
    button_row.set_margin_start(18);
    button_row.set_margin_end(18);

    let dont_show_again = gtk4::CheckButton::with_label(tr!("Don't ask again for this").as_ref());
    dont_show_again.set_valign(gtk4::Align::Center);
    dont_show_again.set_hexpand(true);
    dont_show_again.set_halign(gtk4::Align::Start);
    button_row.append(&dont_show_again);

    let deny_btn = gtk4::Button::with_label(tr!("Deny").as_ref());
    let allow_btn = gtk4::Button::with_label(tr!("Allow").as_ref());
    allow_btn.add_css_class("suggested-action");
    deny_btn.add_css_class("destructive-action");

    let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    button_size_group.add_widget(&deny_btn);
    button_size_group.add_widget(&allow_btn);

    let buttons_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    buttons_box.set_halign(gtk4::Align::End);
    buttons_box.append(&allow_btn);
    buttons_box.append(&deny_btn);
    button_row.append(&buttons_box);
    outer.append(&button_row);

    dialog.set_child(Some(&outer));
    dialog.set_default_widget(Some(&allow_btn));

    // Guards against answering twice: closing the window (any way — a
    // button, Escape, or the titlebar's own close button) always routes
    // through `respond`, and `dialog.close()` itself re-emits
    // `close-request`, which would otherwise send a second, contradictory
    // answer for the same dialog.
    let answered = Rc::new(Cell::new(false));
    add_countdown_badge(&button_row, &answered);
    let respond = {
        let dont_show_again = dont_show_again.clone();
        let showing = showing.clone();
        let answered = answered.clone();
        move |allow: bool| {
            if answered.replace(true) {
                return;
            }
            let dialogs = dialogs.clone();
            let dont_show = dont_show_again.is_active();
            std::thread::spawn(move || {
                if let Err(e) = dialogs.answer(id, allow, dont_show) {
                    log::warn!("failed to answer JDownloader dialog {}: {}", id, e);
                }
            });
            showing.set(false);
        }
    };

    // The titlebar's close button (or any other window-manager close) has
    // no dedicated answer of its own in JDownloader's protocol, so it's
    // treated the same as clicking Deny.
    dialog.connect_close_request({
        let respond = respond.clone();
        move |_| {
            respond(false);
            glib::Propagation::Proceed
        }
    });

    let escape_controller = gtk4::EventControllerKey::new();
    escape_controller.connect_key_pressed({
        let deny_btn = deny_btn.clone();
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                deny_btn.activate();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    dialog.add_controller(escape_controller);

    deny_btn.connect_clicked({
        let respond = respond.clone();
        let dialog = dialog.clone();
        move |_| {
            respond(false);
            dialog.close();
        }
    });
    allow_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            respond(true);
            dialog.close();
        }
    });

    dialog.present();
    allow_btn.grab_focus();

    ShownDialog {
        window: dialog,
        answered,
    }
}

/// Shows JDownloader's "file already exists" dialog, mirroring the real
/// client's `IfFileExistsDialog` (skip/overwrite/auto-rename radio buttons).
/// Answering this one needs an `action` alongside `closereason`/
/// `dontshowagainselected` — see [`JdDialogs::answer_file_exists`] — which
/// the generic Allow/Deny prompt used for every other dialog type has no way
/// to provide, so without this JDownloader silently falls back to skipping
/// the file no matter which button was clicked.
fn show_file_exists(
    parent: &adw::ApplicationWindow,
    id: i64,
    info: &PendingDialogInfo,
    details: FileExistsDetails,
    dialogs: JdDialogs,
    showing: Rc<Cell<bool>>,
) -> ShownDialog {
    // Two different JDownloader Swing dialogs share this exact same
    // RemoteAPI type/contract (`jd.controlling.downloadcontroller.
    // IfFileExistsDialogInterface`): one for a plain download conflict
    // (`jd.controlling.downloadcontroller.IfFileExistsDialog`) and one for
    // an archive-extraction conflict (`org.jdownloader.extensions.
    // extraction.gui.iffileexistsdialog.IfFileExistsDialog`), with
    // different wording/fields and a rename-name field only the extraction
    // one has. The RemoteAPI gives no way to tell them apart directly (both
    // expose the exact same `@Out` properties), so this infers it from
    // `details.incoming`: a download conflict's `filepath` is a queued
    // download's own destination file (so it has a match there); an
    // extraction conflict's `filepath` is a member inside an archive, which
    // never matches a queued download's destination.
    let is_download = details.incoming.is_some();

    let title = if info.title().is_empty() {
        tr!("File already exists").to_string()
    } else {
        info.title().to_string()
    };

    let dialog = gtk4::Window::new();
    dialog.set_transient_for(Some(parent));
    dialog.set_modal(true);
    dialog.set_resizable(false);
    dialog.set_title(Some(&title));

    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(12);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
        crate::gui::icon_key::ICON_QUESTION,
    ));
    icon.set_pixel_size(48);
    icon.set_valign(gtk4::Align::Start);
    content.append(&icon);

    let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    text_box.set_hexpand(true);

    // Matches `_JDT.T.jd_controlling_SingleDownloadController_askexists3()`
    // (download) / `T.T.file_exists_message()` (extraction) verbatim —
    // neither dialog sends its body text over the RemoteAPI (only the
    // window title is an `@Out` property), so it's hardcoded here like
    // JDownloader itself hardcodes it client-side.
    let message = if is_download {
        tr!("There is a problem downloading a file!\nThe file already exists on disk. What do you want to do?")
            .to_string()
    } else {
        tr!("There is a problem extracting an archive.\nThe file to extract already exists on your harddisk. Please choose what to do:")
            .to_string()
    };
    let message_label = gtk4::Label::new(Some(&message));
    message_label.set_halign(gtk4::Align::Start);
    message_label.set_xalign(0.0);
    message_label.set_wrap(true);
    message_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    text_box.append(&message_label);

    let file_name = std::path::Path::new(info.file_path())
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| info.file_path().to_string());

    // Field order/labels match `IfFileExistsDialog.layoutDialogContent()`
    // exactly for whichever variant this is — both share the same
    // Filename/size/Package fields, then diverge on their last one (Hoster
    // for a download conflict, Archive for an extraction conflict).
    let mut fields = vec![
        (tr!("Filename:").to_string(), file_name),
        (
            tr!("New File's size:").to_string(),
            details
                .incoming
                .map(crate::gui::downloads_panel::format_size_jd)
                .unwrap_or_default(),
        ),
        (
            tr!("Existing File's size:").to_string(),
            details
                .existing
                .map(|n| crate::gui::downloads_panel::format_size_jd(n as i64))
                .unwrap_or_default(),
        ),
        (tr!("Package:").to_string(), info.package_name().to_string()),
    ];
    if is_download {
        fields.push((tr!("Hoster:").to_string(), info.host().to_string()));
    } else {
        fields.push((
            tr!("Archive:").to_string(),
            details.archive_name.clone().unwrap_or_default(),
        ));
    }

    let details_grid = gtk4::Grid::new();
    details_grid.set_row_spacing(4);
    details_grid.set_column_spacing(8);
    for (row, (field, value)) in fields.into_iter().enumerate() {
        if value.is_empty() {
            continue;
        }
        let field_label = gtk4::Label::new(Some(&field));
        field_label.set_halign(gtk4::Align::Start);
        field_label.add_css_class("heading");
        details_grid.attach(&field_label, 0, row as i32, 1, 1);
        let value_label = gtk4::Label::new(Some(&value));
        value_label.set_halign(gtk4::Align::Start);
        value_label.set_xalign(0.0);
        value_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        value_label.set_hexpand(true);
        details_grid.attach(&value_label, 1, row as i32, 1, 1);
    }
    text_box.append(&details_grid);

    content.append(&text_box);
    outer.append(&content);

    let radios_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    radios_box.set_margin_top(6);
    radios_box.set_margin_bottom(6);
    radios_box.set_margin_start(18);
    radios_box.set_margin_end(18);

    let skip_radio = gtk4::CheckButton::with_label(tr!("Skip file").as_ref());
    let overwrite_radio = gtk4::CheckButton::with_label(tr!("Overwrite existing file").as_ref());
    overwrite_radio.set_group(Some(&skip_radio));
    let rename_radio = gtk4::CheckButton::with_label(tr!("Rename file").as_ref());
    rename_radio.set_group(Some(&skip_radio));
    // Matches JDownloader's own default when no prior choice is remembered
    // (`IfFileExistsDialog`'s fallback when `CFG_GUI.getLastIfFileExists()`
    // is unset).
    skip_radio.set_active(true);
    radios_box.append(&skip_radio);
    radios_box.append(&overwrite_radio);
    radios_box.append(&rename_radio);
    outer.append(&radios_box);

    // Only the extraction-conflict variant shows a (read-only) rename
    // preview — the download-conflict dialog has no such field at all: its
    // "Rename file" choice happens silently, with no name shown anywhere.
    if !is_download {
        // Read-only: JDownloader's RemoteAPI has no way to receive a custom
        // name back (see `FileExistsDetails`'s doc comment), so this is
        // purely informational — shown greyed out rather than as an
        // editable field the user could be misled into thinking they
        // control. It's still exactly the name JDownloader itself will pick
        // once `Rename file` is answered, since `suggest_new_name` mirrors
        // its own algorithm.
        let new_name_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        new_name_row.set_margin_bottom(6);
        new_name_row.set_margin_start(18);
        new_name_row.set_margin_end(18);
        let new_name_label = gtk4::Label::new(Some(tr!("New Name:").as_ref()));
        new_name_label.add_css_class("heading");
        new_name_row.append(&new_name_label);
        let new_name_entry = gtk4::Entry::new();
        new_name_entry.set_text(&details.suggested_name);
        new_name_entry.set_hexpand(true);
        new_name_entry.set_sensitive(false);
        new_name_row.append(&new_name_entry);
        outer.append(&new_name_row);
    }

    let button_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    button_row.set_margin_top(6);
    button_row.set_margin_bottom(12);
    button_row.set_margin_start(18);
    button_row.set_margin_end(18);

    // `IfFileExistsDialog.getDontShowAgainLabelText()`: the download
    // conflict remembers the choice per-package, the extraction one per
    // archive — different enough in meaning that JDownloader gives them
    // distinct labels rather than sharing one generic string.
    let dont_show_again_label = if is_download {
        tr!("Remember selection for this Package")
    } else {
        tr!("Don't show again for this archive")
    };
    let dont_show_again = gtk4::CheckButton::with_label(dont_show_again_label.as_ref());
    dont_show_again.set_valign(gtk4::Align::Center);
    dont_show_again.set_hexpand(true);
    dont_show_again.set_halign(gtk4::Align::Start);
    button_row.append(&dont_show_again);

    let ok_btn = gtk4::Button::with_label(tr!("OK").as_ref());
    ok_btn.add_css_class("suggested-action");
    let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());

    let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    button_size_group.add_widget(&ok_btn);
    button_size_group.add_widget(&cancel_btn);

    button_row.append(&ok_btn);
    button_row.append(&cancel_btn);
    outer.append(&button_row);

    dialog.set_child(Some(&outer));
    dialog.set_default_widget(Some(&ok_btn));

    // Guards against answering twice: closing the window (any way — a
    // button, Escape, or the titlebar's own close button) always routes
    // through `respond`, and `dialog.close()` itself re-emits
    // `close-request`, which would otherwise send a second, contradictory
    // answer for the same dialog.
    let answered = Rc::new(Cell::new(false));
    add_countdown_badge(&button_row, &answered);
    let respond = {
        let dont_show_again = dont_show_again.clone();
        let showing = showing.clone();
        let overwrite_radio = overwrite_radio.clone();
        let rename_radio = rename_radio.clone();
        let answered = answered.clone();
        move |allow: bool| {
            if answered.replace(true) {
                return;
            }
            // JDownloader itself never reads `action` when the dialog is
            // cancelled — `throwCloseExceptions()` throws before its own
            // watchdog ever calls `getAction()`, so this only matters when
            // `allow` is true (see `JdDialogs::answer_file_exists`).
            let action = if allow {
                if overwrite_radio.is_active() {
                    FileExistsAction::Overwrite
                } else if rename_radio.is_active() {
                    FileExistsAction::Rename
                } else {
                    FileExistsAction::Skip
                }
            } else {
                FileExistsAction::Skip
            };
            let dialogs = dialogs.clone();
            let dont_show = dont_show_again.is_active();
            std::thread::spawn(move || {
                if let Err(e) = dialogs.answer_file_exists(id, allow, action, dont_show) {
                    log::warn!("failed to answer JDownloader dialog {}: {}", id, e);
                }
            });
            showing.set(false);
        }
    };

    // The titlebar's close button (or any other window-manager close) has
    // no dedicated answer of its own in JDownloader's protocol, so it's
    // treated the same as clicking Cancel.
    dialog.connect_close_request({
        let respond = respond.clone();
        move |_| {
            respond(false);
            glib::Propagation::Proceed
        }
    });

    let escape_controller = gtk4::EventControllerKey::new();
    escape_controller.connect_key_pressed({
        let respond = respond.clone();
        let dialog = dialog.clone();
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                respond(false);
                dialog.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    dialog.add_controller(escape_controller);

    ok_btn.connect_clicked({
        let respond = respond.clone();
        let dialog = dialog.clone();
        move |_| {
            respond(true);
            dialog.close();
        }
    });
    cancel_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            respond(false);
            dialog.close();
        }
    });

    dialog.present();
    ok_btn.grab_focus();

    ShownDialog {
        window: dialog,
        answered,
    }
}
