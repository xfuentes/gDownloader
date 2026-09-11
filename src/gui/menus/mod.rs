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
    icon_label_with_accel(icon, label, None, None)
}

/// Same as [`icon_label`], but with a trailing, dimmed shortcut hint —
/// e.g. "Ctrl+L" — rendered the way GTK's own native menu items show their
/// accelerator, via `gtk4::accelerator_get_label`. `accel` is a
/// `gtk4::accelerator_parse`-style string (e.g. `"<Primary>L"`).
///
/// `size_group` (shared across every row of the same popover) equalizes
/// the icon+label portion's width, so shortcut hints of differently-sized
/// rows all start at the same x position — the same column alignment
/// GTK's native menus give their accelerators for free.
pub fn icon_label_with_accel(
    icon: &str,
    label: &str,
    accel: Option<&str>,
    size_group: Option<&gtk4::SizeGroup>,
) -> gtk4::Box {
    let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    bx.set_halign(gtk4::Align::Fill);

    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let img = gtk4::Image::from_gicon(&jd_icon::resolve(icon));
    img.set_pixel_size(16);
    content.append(&img);
    let label_widget = gtk4::Label::new(Some(label));
    label_widget.set_xalign(0.0);
    content.append(&label_widget);
    bx.append(&content);
    if let Some(group) = size_group {
        group.add_widget(&content);
    }

    if let Some((key, mods)) = accel.and_then(gtk4::accelerator_parse) {
        let accel_label = gtk4::Label::new(Some(&gtk4::accelerator_get_label(key, mods)));
        accel_label.add_css_class("dim-label");
        bx.append(&accel_label);
    }
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
pub(crate) fn popdown_ancestor(widget: &impl IsA<gtk4::Widget>) {
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

/// Same as [`action_button`], but with a trailing shortcut hint (see
/// [`icon_label_with_accel`]) — for a menu row whose action has a matching
/// `gtk4::Application::set_accels_for_action` binding elsewhere.
pub fn action_button_with_accel(
    icon: &str,
    label: &str,
    action: &str,
    accel: &str,
    size_group: Option<&gtk4::SizeGroup>,
) -> gtk4::Button {
    let btn = gtk4::Button::builder()
        .child(&icon_label_with_accel(icon, label, Some(accel), size_group))
        .has_frame(false)
        .halign(gtk4::Align::Fill)
        .action_name(action)
        .build();
    btn.connect_clicked(popdown_ancestor);
    btn
}

/// A toggleable popover row matching GNOME/GTK's own convention for a
/// boolean menu item: a trailing checkmark that appears only when active
/// (not a persistent checkbox square, which GTK reserves for radio-style
/// choices) on a flat, full-width, hoverable row. The `gtk4::CheckButton`
/// returned is never shown — it's just the state holder, so callers can
/// keep using `is_active`/`set_active`/`connect_toggled` exactly as if a
/// real checkbox were on screen.
pub fn check_row(label: &str) -> (gtk4::Button, gtk4::CheckButton) {
    let check = gtk4::CheckButton::new();

    let checkmark = gtk4::Image::from_icon_name("object-select-symbolic");
    checkmark.set_visible(false);

    let label_widget = gtk4::Label::new(Some(label));
    label_widget.set_xalign(0.0);
    label_widget.set_hexpand(true);

    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    content.append(&label_widget);
    content.append(&checkmark);

    let row = gtk4::Button::builder()
        .child(&content)
        .has_frame(false)
        .halign(gtk4::Align::Fill)
        .build();

    check.connect_toggled({
        let checkmark = checkmark.clone();
        move |c| checkmark.set_visible(c.is_active())
    });
    row.connect_clicked({
        let check = check.clone();
        move |_| check.set_active(!check.is_active())
    });
    (row, check)
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
