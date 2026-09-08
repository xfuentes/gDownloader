use gtk4::prelude::*;
use gtk4::glib;

use crate::gui::components::config_section_form::ConfigSectionForm;

/// Configuration section: fixed icon on the left, titled separator,
/// optional description and form on the right.
pub struct ConfigSection {
    container: gtk4::Box,
}

impl ConfigSection {
    pub fn new(
        icon_name: &str,
        title: &str,
        description: Option<&str>,
        form: &ConfigSectionForm,
    ) -> Self {
        let container = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        container.set_hexpand(true);
        container.set_halign(gtk4::Align::Fill);

        let icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(icon_name));
        icon.set_pixel_size(32);
        icon.set_valign(gtk4::Align::Start);
        icon.set_halign(gtk4::Align::Center);
        container.append(&icon);

        let right = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        right.set_hexpand(true);

        let header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        header_box.set_valign(gtk4::Align::Center);
        header_box.set_margin_top(5);

        let title_label = gtk4::Label::new(None);
        title_label.set_markup(&format!(
            "<b><u>{}</u></b>",
            glib::markup_escape_text(title)
        ));
        title_label.set_valign(gtk4::Align::Center);
        header_box.append(&title_label);

        let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        separator.set_valign(gtk4::Align::Center);
        separator.set_hexpand(true);
        header_box.append(&separator);

        right.append(&header_box);

        if let Some(desc) = description {
            let desc_label = gtk4::Label::new(Some(desc));
            desc_label.add_css_class("dim-label");
            desc_label.set_xalign(0.0);
            desc_label.set_halign(gtk4::Align::Fill);
            desc_label.set_wrap(true);
            desc_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
            right.append(&desc_label);
        }

        right.append(form.widget());
        container.append(&right);

        Self { container }
    }

    pub fn widget(&self) -> &gtk4::Box {
        &self.container
    }
}
