use gtk4::prelude::*;

pub fn ellipsize_dropdown(model: gtk4::StringList) -> gtk4::DropDown {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
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
            list_item.set_child(Some(&label));
        }
    });
    factory.connect_bind(|_, list_item| {
        if let Some(list_item) = list_item.downcast_ref::<gtk4::ListItem>() {
            if let Some(label) = list_item.child().and_downcast_ref::<gtk4::Label>() {
                if let Some(obj) = list_item.item().and_downcast_ref::<gtk4::StringObject>() {
                    label.set_label(&obj.string());
                }
            }
        }
    });

    let dropdown = gtk4::DropDown::new(Some(model), None::<&gtk4::Expression>);
    dropdown.set_factory(Some(&factory));
    dropdown.set_list_factory(Some(&factory));
    dropdown
}

pub fn select_from_api(dropdown: &gtk4::DropDown, api_values: &[&str], current: &str) {
    if let Some(idx) = api_values.iter().position(|&v| v == current) {
        dropdown.set_selected(idx as u32);
    }
}
