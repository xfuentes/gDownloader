use adw::prelude::*;
use gtk4::glib;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::jd::{JdApi, JdDialogs, PendingDialogInfo};

/// Polls JDownloader's `dialogs` RemoteAPI for dialogs it can't show
/// locally — it runs headless (no Swing/AWT display), since gDownloader
/// provides the GTK UI instead — and shows them as native GTK windows,
/// answering back through the same API. Currently the only known source is
/// EventScripter's "allow this script to run a program?" prompt, but the
/// mechanism is generic to any headless-blocked JDownloader dialog.
pub fn start(api: Arc<JdApi>, window: adw::ApplicationWindow) {
    let dialogs = JdDialogs::new(api);
    // Only one dialog is ever shown at a time (JDownloader itself requires
    // dialogs to be answered oldest-first), so skip ticks while one is up
    // rather than racing a second fetch against it.
    let showing = Rc::new(Cell::new(false));

    glib::source::timeout_add_local(Duration::from_millis(1500), move || {
        if showing.get() {
            return glib::ControlFlow::Continue;
        }

        let (tx, rx) = async_channel::bounded::<Option<(i64, PendingDialogInfo)>>(1);
        let dialogs_fetch = dialogs.clone();
        std::thread::spawn(move || {
            let next = dialogs_fetch
                .list()
                .ok()
                .and_then(|ids| ids.into_iter().next())
                .and_then(|id| dialogs_fetch.get(id).ok().map(|info| (id, info)));
            let _ = tx.try_send(next);
        });

        let showing_c = showing.clone();
        let dialogs_c = dialogs.clone();
        let window_c = window.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(Some((id, info))) = rx.recv().await {
                showing_c.set(true);
                show(&window_c, id, &info, dialogs_c, showing_c);
            }
        });

        glib::ControlFlow::Continue
    });
}

fn show(
    parent: &adw::ApplicationWindow,
    id: i64,
    info: &PendingDialogInfo,
    dialogs: JdDialogs,
    showing: Rc<Cell<bool>>,
) {
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

    let answer = {
        let dialog = dialog.clone();
        let dont_show_again = dont_show_again.clone();
        let showing = showing.clone();
        move |allow: bool| {
            let dialogs = dialogs.clone();
            let dont_show = dont_show_again.is_active();
            std::thread::spawn(move || {
                if let Err(e) = dialogs.answer(id, allow, dont_show) {
                    log::warn!("failed to answer JDownloader dialog {}: {}", id, e);
                }
            });
            dialog.close();
            showing.set(false);
        }
    };

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

    let answer_deny = answer.clone();
    deny_btn.connect_clicked(move |_| answer_deny(false));
    allow_btn.connect_clicked(move |_| answer(true));

    dialog.present();
    allow_btn.grab_focus();
}
