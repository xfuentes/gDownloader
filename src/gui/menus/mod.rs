use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;

use crate::gui::jd_icon;

pub mod file;
pub mod help;
pub mod settings;
pub mod tools;

/// Custom widgets to register on the hosting `PopoverMenu`/`PopoverMenuBar`
/// via `add_child`, paired with the "custom" attribute id they render for.
pub type MenuChildren = Vec<(gtk4::Widget, String)>;

pub fn custom_item(label: &str, detailed_action: Option<&str>, id: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), detailed_action);
    item.set_attribute_value("custom", Some(&glib::Variant::from(id)));
    item
}

pub fn icon_label(icon: &str, label: &str) -> gtk4::Box {
    let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    bx.set_halign(gtk4::Align::Fill);
    let img = gtk4::Image::from_gicon(&jd_icon::resolve(icon));
    img.set_pixel_size(16);
    bx.append(&img);
    bx.append(&gtk4::Label::new(Some(label)));
    bx
}

/// Composes two icons into a single `gtk4::Fixed`, `base` at `(0, 0)` and
/// `overlay` at `(x, y)`, mirroring JDownloader's own merged/badge icons
/// (`ExtMergedIcon`/`BadgeIcon`, e.g. the toolbar toggles' on/off checkbox
/// overlay). Returns the overlay `Image` too, so callers can show/hide it
/// for a badge that reflects live state rather than a fixed composition.
pub fn merged_icon(
    base: &str,
    base_size: i32,
    overlay: &str,
    overlay_size: i32,
    x: f64,
    y: f64,
) -> (gtk4::Fixed, gtk4::Image) {
    let fixed = gtk4::Fixed::new();
    fixed.set_size_request(base_size, base_size);
    let base_img = gtk4::Image::from_gicon(&jd_icon::resolve(base));
    base_img.set_pixel_size(base_size);
    fixed.put(&base_img, 0.0, 0.0);
    let overlay_img = gtk4::Image::from_gicon(&jd_icon::resolve(overlay));
    overlay_img.set_pixel_size(overlay_size);
    fixed.put(&overlay_img, x, y);
    (fixed, overlay_img)
}

pub fn merged_icon_label(
    base: &str,
    base_size: i32,
    overlay: &str,
    overlay_size: i32,
    x: f64,
    y: f64,
    label: &str,
) -> gtk4::Box {
    let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    bx.set_halign(gtk4::Align::Fill);
    let (fixed, _overlay_img) = merged_icon(base, base_size, overlay, overlay_size, x, y);
    bx.append(&fixed);
    bx.append(&gtk4::Label::new(Some(label)));
    bx
}

/// Closes the nearest ancestor popover of `widget`, if any. Custom children
/// added to a `PopoverMenu`/`PopoverMenuBar` via `add_child` are plain
/// buttons and don't auto-close their popover on activation like native
/// menu-model items do, so callers close it by hand after the action runs.
fn popdown_ancestor(widget: &impl IsA<gtk4::Widget>) {
    if let Some(popover) = widget
        .as_ref()
        .ancestor(gtk4::Popover::static_type())
        .and_then(|w| w.downcast::<gtk4::Popover>().ok())
    {
        popover.popdown();
    }
}

pub fn action_button(icon: &str, label: &str, action: &str) -> gtk4::Button {
    let btn = gtk4::Button::builder()
        .child(&icon_label(icon, label))
        .has_frame(false)
        .halign(gtk4::Align::Fill)
        .action_name(action)
        .build();
    btn.connect_clicked(popdown_ancestor);
    btn
}

pub fn merged_action_button(
    base: &str,
    base_size: i32,
    overlay: &str,
    overlay_size: i32,
    x: f64,
    y: f64,
    label: &str,
    action: &str,
) -> gtk4::Button {
    let btn = gtk4::Button::builder()
        .child(&merged_icon_label(
            base, base_size, overlay, overlay_size, x, y, label,
        ))
        .has_frame(false)
        .halign(gtk4::Align::Fill)
        .action_name(action)
        .build();
    btn.connect_clicked(popdown_ancestor);
    btn
}

pub fn disabled_button(icon: &str, label: &str) -> gtk4::Button {
    gtk4::Button::builder()
        .child(&icon_label(icon, label))
        .has_frame(false)
        .halign(gtk4::Align::Fill)
        .sensitive(false)
        .build()
}

pub fn submenu_button(icon: &str, label: &str, popover: &gtk4::PopoverMenu) -> gtk4::MenuButton {
    gtk4::MenuButton::builder()
        .child(&icon_label(icon, label))
        .has_frame(false)
        .direction(gtk4::ArrowType::Right)
        .always_show_arrow(true)
        .popover(popover)
        .build()
}
