// gDownloader
// Copyright (c) 2026. Xavier Fuentes <xfuentes-dev@serviam.cc>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk4::glib;

/// Static definition of one overview stat: the JDownloader
/// `GraphicalUserInterfaceSettings` flag backing its visibility, its label,
/// and whether it's shown by default. Mirrors a `DataEntry` from
/// JDownloader's `AbstractOverviewPanel`, minus its total/filtered/selected
/// modes (this panel only ever shows one value per stat).
pub struct OverviewFieldDef {
    pub key: &'static str,
    pub cfg_key: &'static str,
    pub label: String,
    pub default_visible: bool,
}

impl OverviewFieldDef {
    pub fn new(key: &'static str, cfg_key: &'static str, label: String, default_visible: bool) -> Self {
        Self { key, cfg_key, label, default_visible }
    }
}

/// Fires when the user toggles a field's visibility from the wrench menu, so
/// the caller can persist it via JDownloader's own config — mirrors
/// `properties_panel::PropertiesActions::set_field_visible`.
pub struct OverviewActions {
    pub set_field_visible: Box<dyn Fn(&'static str, bool)>,
}

struct FieldRow {
    def: OverviewFieldDef,
    label_widget: gtk4::Label,
    value_widget: gtk4::Label,
    visible: Cell<bool>,
}

/// Re-attaches every visible field to `grid`, two at a time (label + value)
/// filling row 0 then row 1 before moving to the next column pair — e.g. the
/// first visible field goes to (row 0, col 0), the second to (row 1, col 0),
/// the third to (row 0, col 2), and so on. Mirrors the row1/row2 packing in
/// JDownloader's `AbstractOverviewPanel.layoutInfoPanel`.
fn relayout(grid: &gtk4::Grid, fields: &[Rc<FieldRow>]) {
    for field in fields {
        if field.label_widget.parent().is_some() {
            grid.remove(&field.label_widget);
        }
        if field.value_widget.parent().is_some() {
            grid.remove(&field.value_widget);
        }
    }
    let mut i: i32 = 0;
    for field in fields {
        if !field.visible.get() {
            continue;
        }
        let row = i % 2;
        let col = (i / 2) * 2;
        grid.attach(&field.label_widget, col, row, 1, 1);
        grid.attach(&field.value_widget, col + 1, row, 1, 1);
        i += 1;
    }
}

/// A JDownloader-style "Overview" panel: a two-row grid of label/value stat
/// pairs, with the set of visible stats chosen via a wrench menu in the
/// header — mirrors `AbstractOverviewPanel`/`DataEntry`/`DownloadOverViewHeader`
/// from JDownloader. Shared by the downloads and link grabber panels, each
/// passing its own list of stats and default visibility.
pub struct OverviewPanel {
    pub widget: gtk4::Frame,
    fields: Vec<Rc<FieldRow>>,
    field_checks: Vec<(Rc<FieldRow>, gtk4::CheckButton)>,
    visibility_loading: Rc<Cell<bool>>,
}

impl OverviewPanel {
    pub fn build(title: &str, icon_key: &str, defs: Vec<OverviewFieldDef>, actions: OverviewActions) -> Self {
        let actions = Rc::new(actions);

        // Header: icon + static title on the left, discreet "visible fields"
        // button pinned to the right, same layout as `PropertiesPanel`'s
        // header (a plain full-width box rather than `Frame`'s label-widget
        // slot, which only ever sizes to its natural width).
        let header_icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(icon_key));
        header_icon.set_pixel_size(16);
        let header_label = gtk4::Label::new(Some(title));
        header_label.set_xalign(0.0);
        header_label.set_hexpand(true);
        let fields_btn = gtk4::MenuButton::new();
        let fields_btn_icon =
            gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_WRENCH));
        fields_btn_icon.set_pixel_size(10);
        fields_btn.set_child(Some(&fields_btn_icon));
        fields_btn.set_has_frame(false);
        fields_btn.set_tooltip_text(Some(tr!("Choose visible fields").as_ref()));
        let header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        header_box.set_margin_start(6);
        header_box.set_margin_end(6);
        header_box.set_margin_top(4);
        header_box.set_margin_bottom(4);
        header_box.append(&header_icon);
        header_box.append(&header_label);
        header_box.append(&fields_btn);

        let frame = gtk4::Frame::new(None);
        let panel_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        panel_box.append(&header_box);
        panel_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        // Column gap is the base 6px cell padding plus 1em (~16px at the
        // default UI font size) of extra breathing room between stats.
        let grid = gtk4::Grid::new();
        grid.set_row_spacing(4);
        grid.set_column_spacing(22);
        grid.set_margin_start(6);
        grid.set_margin_end(6);
        grid.set_margin_top(6);
        grid.set_margin_bottom(6);

        let mut fields: Vec<Rc<FieldRow>> = Vec::new();
        for def in defs {
            let label_widget = gtk4::Label::new(Some(&format!("{}:", def.label)));
            label_widget.set_xalign(1.0);
            label_widget.set_halign(gtk4::Align::End);
            label_widget.add_css_class("caption");
            let value_widget = gtk4::Label::new(Some("-"));
            value_widget.set_xalign(0.0);
            value_widget.set_halign(gtk4::Align::Start);
            value_widget.add_css_class("body");
            let visible = Cell::new(def.default_visible);
            fields.push(Rc::new(FieldRow { def, label_widget, value_widget, visible }));
        }
        relayout(&grid, &fields);

        // "Visible fields" popover, listing a checkbutton per field. Toggling
        // one relayouts the grid and persists it via
        // `actions.set_field_visible`, unless the change is being applied
        // programmatically (see `set_field_visible` below).
        let visibility_loading = Rc::new(Cell::new(false));
        let fields_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        fields_box.set_margin_start(8);
        fields_box.set_margin_end(8);
        fields_box.set_margin_top(8);
        fields_box.set_margin_bottom(8);
        let mut field_checks: Vec<(Rc<FieldRow>, gtk4::CheckButton)> = Vec::new();
        for field in &fields {
            let check = gtk4::CheckButton::with_label(&field.def.label);
            check.set_active(field.visible.get());
            check.connect_toggled({
                let field = field.clone();
                let actions = actions.clone();
                let visibility_loading = visibility_loading.clone();
                let grid = grid.clone();
                let fields = fields.clone();
                move |c| {
                    field.visible.set(c.is_active());
                    relayout(&grid, &fields);
                    if !visibility_loading.get() {
                        (actions.set_field_visible)(field.def.key, c.is_active());
                    }
                }
            });
            fields_box.append(&check);
            field_checks.push((field.clone(), check));
        }
        let fields_popover = gtk4::Popover::new();
        fields_popover.set_child(Some(&fields_box));
        fields_btn.set_popover(Some(&fields_popover));

        panel_box.append(&grid);
        frame.set_child(Some(&panel_box));

        Self { widget: frame, fields, field_checks, visibility_loading }
    }

    /// Updates a stat's displayed value (its label stays fixed).
    pub fn set_value(&self, key: &str, text: &str) {
        if let Some(field) = self.fields.iter().find(|f| f.def.key == key) {
            field.value_widget.set_text(text);
        }
    }

    /// Applies a previously-persisted field visibility (fetched from
    /// JDownloader's config), without triggering `actions.set_field_visible`
    /// back. Assumes the grid it's attached to hasn't been rebuilt since.
    pub fn set_field_visible(&self, key: &str, visible: bool) {
        if let Some((field, check)) = self.field_checks.iter().find(|(f, _)| f.def.key == key) {
            self.visibility_loading.set(true);
            field.visible.set(visible);
            check.set_active(visible);
            self.visibility_loading.set(false);
        }
    }
}

/// Waits (up to 30s) for `is_ready`, then applies the field-visibility
/// values returned by `fetch` to `overview`. `fetch` runs on a background
/// thread since it blocks on JDownloader's config API. Mirrors
/// `properties_panel::restore_field_visibility`.
pub fn restore_field_visibility(
    overview: Rc<OverviewPanel>,
    is_ready: impl Fn() -> bool + Send + 'static,
    fetch: impl FnOnce() -> Vec<(&'static str, bool)> + Send + 'static,
) {
    let (tx, rx) = async_channel::bounded::<Vec<(&'static str, bool)>>(1);
    std::thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(30) {
            if is_ready() {
                let _ = tx.send_blocking(fetch());
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(values) = rx.recv().await {
            for (key, visible) in values {
                overview.set_field_visible(key, visible);
            }
        }
    });
}
