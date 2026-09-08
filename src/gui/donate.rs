use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;

/// Shows the borderless JDownloader donation dialog.
pub fn show(parent: &adw::ApplicationWindow) {
    let now = glib::DateTime::now_local();
    let year = now.map(|d| d.year()).unwrap_or(2025);
    let years = (year - 2007).to_string();

    let title = tr!("The JDownloader project needs your help!");
    let body = tr!(
        "If you are a satisfied user of JDownloader, please think about contributing to this project. JDownloader is the result of daily hard work since more than %s1 years. We need your help to keep it free of charge, free of advertising, free of installer bundles and to improve JDownloader even more. Moreover, donating is a good way to tell us what modules we should focus our work on."
    )
    .replace("%s1", &years);

    let icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_HEART));
    icon.set_pixel_size(32);
    icon.set_halign(gtk4::Align::Start);
    icon.set_valign(gtk4::Align::Center);

    let text = gtk4::Label::new(Some(&body));
    text.set_wrap(true);
    text.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    text.set_halign(gtk4::Align::Fill);
    text.set_hexpand(true);
    text.set_xalign(0.0);

    let body_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    body_row.set_halign(gtk4::Align::Fill);
    body_row.append(&icon);
    body_row.append(&text);

    let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());
    cancel_btn.set_hexpand(false);
    cancel_btn.set_halign(gtk4::Align::Center);

    let continue_btn = gtk4::Button::with_label(tr!("Continue").as_ref());
    continue_btn.set_hexpand(false);
    continue_btn.set_halign(gtk4::Align::Center);
    continue_btn.add_css_class("suggested-action");

    let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    button_size_group.add_widget(&cancel_btn);
    button_size_group.add_widget(&continue_btn);

    let button_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    button_row.set_halign(gtk4::Align::End);
    button_row.set_valign(gtk4::Align::End);
    button_row.append(&continue_btn);
    button_row.append(&cancel_btn);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 24);
    content.set_halign(gtk4::Align::Center);
    content.set_valign(gtk4::Align::Center);
    content.set_vexpand(false);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.set_spacing(12);
    content.append(&body_row);
    content.append(&button_row);

    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    page.set_vexpand(true);
    page.set_hexpand(true);
    page.set_halign(gtk4::Align::Fill);
    page.set_valign(gtk4::Align::Fill);
    page.append(&content);

    let dialog = gtk4::Window::new();
    dialog.set_title(Some(&title));
    dialog.set_transient_for(Some(parent));
    dialog.set_modal(true);
    dialog.set_default_size(800, -1);
    dialog.set_resizable(false);
    dialog.set_child(Some(&page));
    dialog.set_default_widget(Some(&continue_btn));

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

    let parent = parent.clone();
    cancel_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });

    continue_btn.connect_clicked({
        let dialog = dialog.clone();
        let parent = parent.clone();
        move |_| {
            dialog.close();
            let launcher = gtk4::UriLauncher::new(
                "https://my.jdownloader.org/contribute/#/?ref=jdownloader",
            );
            let _ = launcher.launch(Some(&parent), None::<&gio::Cancellable>, |_| {});
        }
    });

    dialog.present();
    continue_btn.grab_focus();
}
