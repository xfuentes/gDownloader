use gtk4::glib;
use gtk4::prelude::*;

/// Builds the closed-button-face label: compact, ellipsizing with a
/// tooltip fallback, since the closed dropdown has limited width.
fn ellipsized_label() -> gtk4::Label {
    let label = gtk4::Label::new(None);
    label.set_xalign(0.0);
    label.set_halign(gtk4::Align::Fill);
    label.set_hexpand(true);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_has_tooltip(true);
    label.connect_query_tooltip(|label, _x, _y, _keyboard, tooltip| {
        let text = label.label();
        if label.layout().is_ellipsized() {
            tooltip.set_text(Some(&text));
            true
        } else {
            false
        }
    });
    label
}

pub fn ellipsize_dropdown(model: gtk4::StringList) -> gtk4::DropDown {
    // Button face: compact, ellipsizes (see `ellipsized_label`).
    let button_factory = gtk4::SignalListItemFactory::new();
    button_factory.connect_setup(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
            list_item.set_child(Some(&ellipsized_label()));
        }
    });
    button_factory.connect_bind(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
            if let Some(label) = list_item.child().and_downcast_ref::<gtk4::Label>() {
                if let Some(obj) = list_item.item().and_downcast_ref::<gtk4::StringObject>() {
                    label.set_label(&obj.string());
                }
            }
        }
    });

    // Popup list: no ellipsize/hexpand, so each row — and the popover
    // itself — sizes to its own full text instead of being clipped to
    // match the closed button's (possibly much narrower) width.
    let list_factory = gtk4::SignalListItemFactory::new();
    list_factory.connect_setup(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
            let label = gtk4::Label::new(None);
            label.set_xalign(0.0);
            list_item.set_child(Some(&label));
        }
    });
    list_factory.connect_bind(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
            if let Some(label) = list_item.child().and_downcast_ref::<gtk4::Label>() {
                if let Some(obj) = list_item.item().and_downcast_ref::<gtk4::StringObject>() {
                    label.set_label(&obj.string());
                }
            }
        }
    });

    let dropdown = gtk4::DropDown::new(Some(model), None::<&gtk4::Expression>);
    dropdown.set_factory(Some(&button_factory));
    dropdown.set_list_factory(Some(&list_factory));
    dropdown
}

/// Like [`ellipsize_dropdown`], but with a fixed icon in front of each
/// row's label — both in the popup list and in the closed button — for
/// comboboxes that always show an icon per entry, mirroring JDownloader's
/// own Priority combobox (`Priority#getIcon()`). `icons[i]` (an
/// [`crate::gui::icon_key`] constant) is shown for `model`'s i-th entry.
pub fn icon_dropdown(model: gtk4::StringList, icons: Vec<&'static str>) -> gtk4::DropDown {
    let icons = std::rc::Rc::new(icons);

    let bind_row = {
        let icons = icons.clone();
        move |_: &gtk4::SignalListItemFactory, list_item: &glib::Object| {
            let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() else {
                return;
            };
            let pos = list_item.position() as usize;
            let Some(row) = list_item.child().and_downcast::<gtk4::Box>() else {
                return;
            };
            if let Some(icon_widget) = row.first_child().and_downcast::<gtk4::Image>() {
                if let Some(&key) = icons.get(pos) {
                    icon_widget.set_from_gicon(&crate::gui::jd_icon::resolve(key));
                }
            }
            if let Some(label_widget) = row.last_child().and_downcast::<gtk4::Label>() {
                if let Some(obj) = list_item.item().and_downcast::<gtk4::StringObject>() {
                    label_widget.set_label(&obj.string());
                }
            }
        }
    };

    // Button face: compact, ellipsizes (limited width in the closed state).
    let button_factory = gtk4::SignalListItemFactory::new();
    button_factory.connect_setup(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            row.set_halign(gtk4::Align::Fill);
            let icon = gtk4::Image::new();
            icon.set_pixel_size(16);
            row.append(&icon);
            row.append(&ellipsized_label());
            list_item.set_child(Some(&row));
        }
    });
    button_factory.connect_bind(bind_row.clone());

    // Popup list: no ellipsize/hexpand, so each row — and the popover
    // itself — sizes to its own full text instead of being clipped to
    // match the closed button's (possibly much narrower) width.
    let list_factory = gtk4::SignalListItemFactory::new();
    list_factory.connect_setup(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            let icon = gtk4::Image::new();
            icon.set_pixel_size(16);
            let label = gtk4::Label::new(None);
            label.set_xalign(0.0);
            row.append(&icon);
            row.append(&label);
            list_item.set_child(Some(&row));
        }
    });
    list_factory.connect_bind(bind_row);

    let dropdown = gtk4::DropDown::new(Some(model), None::<&gtk4::Expression>);
    dropdown.set_factory(Some(&button_factory));
    dropdown.set_list_factory(Some(&list_factory));
    dropdown
}

pub fn select_from_api(dropdown: &gtk4::DropDown, api_values: &[&str], current: &str) {
    if let Some(idx) = api_values.iter().position(|&v| v == current) {
        dropdown.set_selected(idx as u32);
    }
}
