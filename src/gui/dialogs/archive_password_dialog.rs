use gtk4::glib;
use gtk4::prelude::*;

/// Prompts for a password to apply to the selected archive(s), mirroring
/// the context menu's "Set Archive Password"
/// (`SetExtractPasswordAction`/`ArchivesSubMenu`).
pub struct ArchivePasswordDialog;

impl ArchivePasswordDialog {
    pub fn show(parent: &gtk4::Window, on_confirm: impl Fn(String) + 'static) {
        let dialog = gtk4::Window::new();
        dialog.set_transient_for(Some(parent));
        dialog.set_modal(true);
        dialog.set_resizable(false);
        dialog.set_title(Some(tr!("Set Archive Password").as_ref()));

        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        vbox.set_margin_top(18);
        vbox.set_margin_bottom(18);
        vbox.set_margin_start(18);
        vbox.set_margin_end(18);

        let label = gtk4::Label::new(Some(tr!("Password").as_ref()));
        label.set_halign(gtk4::Align::Start);
        vbox.append(&label);

        let entry = gtk4::Entry::new();
        entry.set_visibility(false);
        entry.set_hexpand(true);
        entry.set_activates_default(true);
        vbox.append(&entry);

        let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());
        let ok_btn = gtk4::Button::with_label(tr!("OK").as_ref());
        ok_btn.add_css_class("suggested-action");

        let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        button_size_group.add_widget(&cancel_btn);
        button_size_group.add_widget(&ok_btn);

        let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        button_box.set_halign(gtk4::Align::End);
        button_box.append(&ok_btn);
        button_box.append(&cancel_btn);
        vbox.append(&button_box);

        dialog.set_child(Some(&vbox));
        dialog.set_default_widget(Some(&ok_btn));

        let escape_controller = gtk4::EventControllerKey::new();
        escape_controller.connect_key_pressed({
            let cancel_btn = cancel_btn.clone();
            move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    cancel_btn.activate();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        });
        dialog.add_controller(escape_controller);

        cancel_btn.connect_clicked({
            let dialog = dialog.clone();
            move |_| dialog.close()
        });
        ok_btn.connect_clicked({
            let dialog = dialog.clone();
            let entry = entry.clone();
            move |_| {
                let password = entry.text().to_string();
                if !password.is_empty() {
                    on_confirm(password);
                }
                dialog.close();
            }
        });

        dialog.present();
        entry.grab_focus();
    }
}
