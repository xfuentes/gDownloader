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

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;

/// Data displayed/edited by a [`PropertiesPanel`] for a single selected link,
/// mirroring JDownloader's "Package or Link Properties" panel.
#[derive(Clone, Debug)]
pub struct PropertiesData {
    pub link_id: i64,
    pub package_id: i64,
    pub file_name: String,
    pub file_icon: gio::Icon,
    pub package_name: String,
    pub save_to: String,
    pub url: String,
    pub comment: String,
    pub priority: String,
}

impl PartialEq for PropertiesData {
    fn eq(&self, other: &Self) -> bool {
        self.link_id == other.link_id
            && self.package_id == other.package_id
            && self.file_name == other.file_name
            && self.package_name == other.package_name
            && self.save_to == other.save_to
            && self.url == other.url
            && self.comment == other.comment
            && self.priority == other.priority
    }
}

/// Fires the actual JDownloader API calls when a field is edited. Each
/// closure is fire-and-forget: the panel does not wait for a response, the
/// next periodic refresh reconciles the authoritative state.
pub struct PropertiesActions {
    pub rename_link: Box<dyn Fn(i64, String)>,
    pub rename_package: Box<dyn Fn(i64, String)>,
    pub set_comment: Box<dyn Fn(i64, String)>,
    pub set_priority: Box<dyn Fn(i64, String)>,
    pub set_download_directory: Box<dyn Fn(i64, String)>,
    /// Called when the user toggles a field's visibility from the "visible
    /// fields" menu, so the caller can persist it via JDownloader's own
    /// config. `key` is one of `FIELD_PACKAGE_NAME`, `FIELD_FILE_NAME`,
    /// `FIELD_SAVE_TO`, `FIELD_SOURCE`, `FIELD_COMMENT` (which also covers
    /// the priority dropdown, shown/hidden together with the comment field
    /// as JDownloader itself does since they share a row).
    pub set_field_visible: Box<dyn Fn(&'static str, bool)>,
}

pub const FIELD_PACKAGE_NAME: &str = "package_name";
pub const FIELD_FILE_NAME: &str = "file_name";
pub const FIELD_SAVE_TO: &str = "save_to";
pub const FIELD_SOURCE: &str = "source";
pub const FIELD_COMMENT: &str = "comment";

/// `Priority` enum names, in JDownloader's own declaration order.
pub(crate) const PRIORITIES: [&str; 7] = [
    "HIGHEST", "HIGHER", "HIGH", "DEFAULT", "LOW", "LOWER", "LOWEST",
];

/// Icon shown for each [`PRIORITIES`] entry, matching JDownloader's own
/// `Priority#getIcon()`.
pub(crate) fn priority_icons() -> Vec<&'static str> {
    PRIORITIES
        .iter()
        .map(|key| match *key {
            "HIGHEST" => crate::gui::icon_key::ICON_PRIO_3,
            "HIGHER" => crate::gui::icon_key::ICON_PRIO_2,
            "HIGH" => crate::gui::icon_key::ICON_PRIO_1,
            "LOW" => crate::gui::icon_key::ICON_PRIO_MINUS_1,
            "LOWER" => crate::gui::icon_key::ICON_PRIO_MINUS_2,
            "LOWEST" => crate::gui::icon_key::ICON_PRIO_MINUS_3,
            _ => crate::gui::icon_key::ICON_PRIO_0,
        })
        .collect()
}

/// JDownloader folds the field name into the dropdown entry itself (e.g.
/// "Default priority") instead of a separate "Priority:" label.
pub(crate) fn priority_label(key: &str) -> String {
    match key {
        "HIGHEST" => tr!("Highest priority").to_string(),
        "HIGHER" => tr!("Higher priority").to_string(),
        "HIGH" => tr!("High priority").to_string(),
        "LOW" => tr!("Low priority").to_string(),
        "LOWER" => tr!("Lower priority").to_string(),
        "LOWEST" => tr!("Lowest priority").to_string(),
        _ => tr!("Default priority").to_string(),
    }
}

/// A single labeled field row: the label and its input widget(s) are toggled
/// together by the "visible fields" menu.
struct FieldRow {
    key: &'static str,
    toggle_label: String,
    widgets: Vec<gtk4::Widget>,
    visible: Cell<bool>,
}

impl FieldRow {
    fn new(key: &'static str, toggle_label: &str, widgets: Vec<gtk4::Widget>) -> Self {
        Self {
            key,
            toggle_label: toggle_label.to_string(),
            widgets,
            visible: Cell::new(true),
        }
    }

    fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        for w in &self.widgets {
            w.set_visible(visible);
        }
    }
}

/// Wires `entry` to call `commit(text)` when the user presses Enter or moves
/// focus away, but only when the panel isn't currently being repopulated and
/// the text actually changed.
fn on_entry_commit(
    entry: &gtk4::Entry,
    loading: Rc<Cell<bool>>,
    last: Rc<RefCell<String>>,
    commit: Rc<dyn Fn(String)>,
) {
    let fire = {
        let entry = entry.clone();
        let loading = loading.clone();
        let last = last.clone();
        let commit = commit.clone();
        move || {
            if loading.get() {
                return;
            }
            let text = entry.text().to_string();
            if text != *last.borrow() {
                last.replace(text.clone());
                commit(text);
            }
        }
    };
    entry.connect_activate({
        let fire = fire.clone();
        move |_| fire()
    });
    let focus = gtk4::EventControllerFocus::new();
    focus.connect_leave(move |_| fire());
    entry.add_controller(focus);
}

pub struct PropertiesPanel {
    pub widget: gtk4::Revealer,
    header_icon: gtk4::Image,
    header_label: gtk4::Label,
    file_name: gtk4::Entry,
    package_name: gtk4::Entry,
    save_to: gtk4::Entry,
    source: gtk4::Entry,
    comment: gtk4::Entry,
    priority: gtk4::DropDown,
    current: Rc<RefCell<Option<PropertiesData>>>,
    /// Set while `update()` is repopulating fields, so the change handlers
    /// wired below don't mistake a programmatic update for a user edit.
    loading: Rc<Cell<bool>>,
    field_checks: Vec<(Rc<FieldRow>, gtk4::CheckButton)>,
    visibility_loading: Rc<Cell<bool>>,
}

impl PropertiesPanel {
    pub fn build(actions: PropertiesActions) -> Self {
        let actions = Rc::new(actions);
        let loading = Rc::new(Cell::new(false));
        let current: Rc<RefCell<Option<PropertiesData>>> = Rc::new(RefCell::new(None));

        let revealer = gtk4::Revealer::new();
        revealer.set_transition_type(gtk4::RevealerTransitionType::SlideUp);
        revealer.set_reveal_child(false);

        // Header: file-type icon + dynamic title on the left, discreet
        // "visible fields" button pinned all the way to the right. Built as
        // a plain full-width box (not `Frame`'s label-widget slot, which
        // only ever sizes to its natural width) so the button truly reaches
        // the right edge.
        let header_icon = gtk4::Image::new();
        header_icon.set_pixel_size(16);
        let header_label = gtk4::Label::new(None);
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

        let grid = gtk4::Grid::new();
        grid.set_row_spacing(4);
        grid.set_column_spacing(6);
        grid.set_margin_start(6);
        grid.set_margin_end(6);
        grid.set_margin_top(6);
        grid.set_margin_bottom(6);

        let label = |text: &str| {
            let l = gtk4::Label::new(Some(text));
            l.set_xalign(0.0);
            l.set_halign(gtk4::Align::Start);
            l
        };

        let mut fields: Vec<Rc<FieldRow>> = Vec::new();

        let package_name = gtk4::Entry::new();
        package_name.set_hexpand(true);
        let package_name_label = label(tr!("Package name:").as_ref());
        grid.attach(&package_name_label, 0, 0, 1, 1);
        grid.attach(&package_name, 1, 0, 3, 1);
        fields.push(Rc::new(FieldRow::new(
            FIELD_PACKAGE_NAME,
            tr!("Package name").as_ref(),
            vec![
                package_name_label.clone().upcast(),
                package_name.clone().upcast(),
            ],
        )));

        let file_name = gtk4::Entry::new();
        file_name.set_hexpand(true);
        let file_name_label = label(tr!("File name:").as_ref());
        grid.attach(&file_name_label, 0, 1, 1, 1);
        grid.attach(&file_name, 1, 1, 3, 1);
        fields.push(Rc::new(FieldRow::new(
            FIELD_FILE_NAME,
            tr!("File name").as_ref(),
            vec![file_name_label.clone().upcast(), file_name.clone().upcast()],
        )));

        let save_to = gtk4::Entry::new();
        save_to.set_hexpand(true);
        let browse_btn = gtk4::Button::with_label(tr!("Browse…").as_ref());
        let save_to_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        save_to_box.add_css_class("linked");
        save_to_box.set_hexpand(true);
        save_to_box.append(&save_to);
        save_to_box.append(&browse_btn);
        let save_to_label = label(tr!("Save to:").as_ref());
        grid.attach(&save_to_label, 0, 2, 1, 1);
        grid.attach(&save_to_box, 1, 2, 3, 1);
        fields.push(Rc::new(FieldRow::new(
            FIELD_SAVE_TO,
            tr!("Save to").as_ref(),
            vec![save_to_label.clone().upcast(), save_to_box.clone().upcast()],
        )));

        let source = gtk4::Entry::new();
        source.set_hexpand(true);
        source.set_editable(false);
        let source_label = label(tr!("Source:").as_ref());
        grid.attach(&source_label, 0, 3, 1, 1);
        grid.attach(&source, 1, 3, 3, 1);
        fields.push(Rc::new(FieldRow::new(
            FIELD_SOURCE,
            tr!("Source").as_ref(),
            vec![source_label.clone().upcast(), source.clone().upcast()],
        )));

        let comment = gtk4::Entry::new();
        comment.set_hexpand(true);
        let comment_label = label(tr!("Comment:").as_ref());
        grid.attach(&comment_label, 0, 4, 1, 1);
        grid.attach(&comment, 1, 4, 2, 1);

        // JDownloader shows/hides the priority dropdown together with the
        // comment field, as one "Comment" toggle, since they share a row.
        let priority_model = gtk4::StringList::new(&[]);
        for key in PRIORITIES {
            priority_model.append(&priority_label(key));
        }
        let priority = crate::gui::fields::icon_dropdown(priority_model, priority_icons());
        priority.set_valign(gtk4::Align::Center);
        priority.set_margin_top(0);
        priority.set_margin_bottom(0);
        priority.set_size_request(-1, 18);
        grid.attach(&priority, 3, 4, 1, 1);
        fields.push(Rc::new(FieldRow::new(
            FIELD_COMMENT,
            tr!("Comment").as_ref(),
            vec![
                comment_label.clone().upcast(),
                comment.clone().upcast(),
                priority.clone().upcast(),
            ],
        )));

        // "Visible fields" popover, listing a checkbutton per field. Toggling
        // one persists it via `actions.set_field_visible` unless the change
        // is being applied programmatically (see `set_field_visible` below).
        let visibility_loading = Rc::new(Cell::new(false));
        let fields_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        fields_box.set_margin_start(8);
        fields_box.set_margin_end(8);
        fields_box.set_margin_top(8);
        fields_box.set_margin_bottom(8);
        let mut field_checks: Vec<(Rc<FieldRow>, gtk4::CheckButton)> = Vec::new();
        for field in &fields {
            let check = gtk4::CheckButton::with_label(&field.toggle_label);
            check.set_active(field.visible.get());
            check.connect_toggled({
                let field = field.clone();
                let actions = actions.clone();
                let visibility_loading = visibility_loading.clone();
                move |c| {
                    field.set_visible(c.is_active());
                    if !visibility_loading.get() {
                        (actions.set_field_visible)(field.key, c.is_active());
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
        revealer.set_child(Some(&frame));

        // Package name.
        {
            let last = Rc::new(RefCell::new(String::new()));
            let actions = actions.clone();
            let current = current.clone();
            on_entry_commit(
                &package_name,
                loading.clone(),
                last,
                Rc::new(move |text| {
                    if let Some(data) = current.borrow().as_ref() {
                        (actions.rename_package)(data.package_id, text);
                    }
                }),
            );
        }
        // File name.
        {
            let last = Rc::new(RefCell::new(String::new()));
            let actions = actions.clone();
            let current = current.clone();
            on_entry_commit(
                &file_name,
                loading.clone(),
                last,
                Rc::new(move |text| {
                    if let Some(data) = current.borrow().as_ref() {
                        (actions.rename_link)(data.link_id, text);
                    }
                }),
            );
        }
        // Save to (directory), also editable via the Browse… button.
        {
            let last = Rc::new(RefCell::new(String::new()));
            let actions = actions.clone();
            let current = current.clone();
            on_entry_commit(
                &save_to,
                loading.clone(),
                last,
                Rc::new(move |text| {
                    if let Some(data) = current.borrow().as_ref() {
                        (actions.set_download_directory)(data.package_id, text);
                    }
                }),
            );
        }
        {
            let save_to = save_to.clone();
            browse_btn.connect_clicked(move |_| {
                let dialog = gtk4::FileDialog::new();
                dialog.set_title(tr!("Select Folder").as_ref());
                let current = save_to.text();
                if !current.is_empty() && std::path::Path::new(current.as_str()).is_dir() {
                    dialog.set_initial_folder(Some(&gio::File::for_path(current.as_str())));
                }
                let save_to = save_to.clone();
                glib::MainContext::default().spawn_local(async move {
                    if let Ok(file) = dialog.select_folder_future(None::<&gtk4::Window>).await {
                        if let Some(path) = file.path() {
                            save_to.set_text(&path.to_string_lossy());
                            save_to.emit_activate();
                        }
                    }
                });
            });
        }
        // Comment.
        {
            let last = Rc::new(RefCell::new(String::new()));
            let actions = actions.clone();
            let current = current.clone();
            on_entry_commit(
                &comment,
                loading.clone(),
                last,
                Rc::new(move |text| {
                    if let Some(data) = current.borrow().as_ref() {
                        (actions.set_comment)(data.link_id, text);
                    }
                }),
            );
        }
        // Priority.
        {
            let loading = loading.clone();
            let actions = actions.clone();
            let current = current.clone();
            priority.connect_selected_notify(move |dd| {
                if loading.get() {
                    return;
                }
                let Some(key) = PRIORITIES.get(dd.selected() as usize) else {
                    return;
                };
                if let Some(data) = current.borrow().as_ref() {
                    (actions.set_priority)(data.link_id, key.to_string());
                }
            });
        }

        Self {
            widget: revealer,
            header_icon,
            header_label,
            file_name,
            package_name,
            save_to,
            source,
            comment,
            priority,
            current,
            loading,
            field_checks,
            visibility_loading,
        }
    }

    /// Applies a previously-persisted field visibility (fetched from
    /// JDownloader's config or gDownloader's local config), without
    /// triggering `actions.set_field_visible` back.
    pub fn set_field_visible(&self, key: &str, visible: bool) {
        if let Some((field, check)) = self.field_checks.iter().find(|(f, _)| f.key == key) {
            self.visibility_loading.set(true);
            field.set_visible(visible);
            check.set_active(visible);
            self.visibility_loading.set(false);
        }
    }

    /// Shows the panel populated with `data`, or hides it when `None`
    /// (matching JDownloader: the panel is hidden when nothing is selected).
    pub fn update(&self, data: Option<PropertiesData>) {
        if *self.current.borrow() == data {
            return;
        }
        self.loading.set(true);
        match &data {
            Some(d) => {
                self.header_icon.set_from_gicon(&d.file_icon);
                self.header_label
                    .set_text(&tr!("File Properties: {}", d.file_name));
                self.file_name.set_text(&d.file_name);
                self.package_name.set_text(&d.package_name);
                self.save_to.set_text(&d.save_to);
                self.source.set_text(&d.url);
                self.comment.set_text(&d.comment);
                let idx = PRIORITIES.iter().position(|k| *k == d.priority).unwrap_or(3);
                self.priority.set_selected(idx as u32);
                self.widget.set_reveal_child(true);
            }
            None => {
                self.widget.set_reveal_child(false);
            }
        }
        *self.current.borrow_mut() = data;
        self.loading.set(false);
    }
}

/// Waits (up to 30s) for `is_ready`, then applies the field-visibility
/// values returned by `fetch` to `properties`. `fetch` runs on a background
/// thread since it blocks on JDownloader's config API. Mirrors the
/// "restore settings on startup" pattern used elsewhere in the app.
pub fn restore_field_visibility(
    properties: Rc<PropertiesPanel>,
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
                properties.set_field_visible(key, visible);
            }
        }
    });
}
