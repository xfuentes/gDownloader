use gtk4::prelude::*;
use std::cell::Cell;

/// Configuration field grid. The three columns (label, checkbox, input) are
/// homogenized with the other forms on the same page via shared `SizeGroup`s.
pub struct ConfigSectionForm {
    grid: gtk4::Grid,
    next_row: Cell<i32>,
    label_size_group: gtk4::SizeGroup,
    checkbox_size_group: gtk4::SizeGroup,
    input_size_group: gtk4::SizeGroup,
}

impl ConfigSectionForm {
    pub fn new(
        label_size_group: &gtk4::SizeGroup,
        checkbox_size_group: &gtk4::SizeGroup,
        input_size_group: &gtk4::SizeGroup,
    ) -> Self {
        let grid = gtk4::Grid::new();
        grid.set_column_spacing(12);
        grid.set_row_spacing(6);
        grid.set_hexpand(true);
        grid.set_halign(gtk4::Align::Fill);

        Self {
            grid,
            next_row: Cell::new(0),
            label_size_group: label_size_group.clone(),
            checkbox_size_group: checkbox_size_group.clone(),
            input_size_group: input_size_group.clone(),
        }
    }

    /// Adds a row and returns every widget the row is made of (label,
    /// checkbox-or-placeholder, input container), so a caller that needs to
    /// hide the whole row later (e.g. a settings field that only applies to
    /// some choice of another field) can do so — a `GtkGrid` collapses a
    /// row's height on its own once every widget in it is invisible.
    pub fn add_row<'a>(
        &self,
        label: impl Into<Option<&'a str>>,
        checkbox: Option<&gtk4::Switch>,
        input: &impl IsA<gtk4::Widget>,
    ) -> Vec<gtk4::Widget> {
        let row = self.next_row.get();
        let label = label.into();
        let mut row_widgets: Vec<gtk4::Widget> = Vec::with_capacity(3);

        let mut next_col = 0;
        let mut input_col = 0;
        let mut input_width = 3;

        if let Some(text) = label {
            let label = gtk4::Label::new(Some(text));
            label.set_max_width_chars(40);
            label.set_xalign(0.0);
            label.set_valign(gtk4::Align::Center);
            label.set_halign(gtk4::Align::Fill);
            label.set_hexpand(true);
            label.set_wrap(true);
            label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
            self.label_size_group.add_widget(&label);
            self.grid.attach(&label, 0, row, 1, 1);
            row_widgets.push(label.upcast());
            next_col = 1;
            input_col = 1;
            input_width = 2;
        }

        if let Some(cb) = checkbox {
            cb.set_valign(gtk4::Align::Center);
            cb.set_hexpand(false);
            cb.set_halign(gtk4::Align::Center);
            self.checkbox_size_group.add_widget(cb);
            self.grid.attach(cb, next_col, row, 1, 1);
            row_widgets.push(cb.clone().upcast());
            next_col += 1;
            input_col = next_col;
            input_width = 3 - next_col;
        } else if label.is_some() {
            let placeholder = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
            placeholder.set_valign(gtk4::Align::Center);
            placeholder.set_hexpand(false);
            placeholder.set_halign(gtk4::Align::Center);
            self.checkbox_size_group.add_widget(&placeholder);
            self.grid.attach(&placeholder, next_col, row, 1, 1);
            row_widgets.push(placeholder.upcast());
            next_col += 1;
            input_col = next_col;
            input_width = 3 - next_col;
        }

        let input_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        input_box.set_valign(gtk4::Align::Center);
        input_box.set_hexpand(true);
        input_box.set_halign(gtk4::Align::Fill);

        let input_widget = input.clone();
        input_widget.set_valign(gtk4::Align::Center);
        if input.is::<gtk4::Switch>() {
            input_widget.set_hexpand(false);
            input_widget.set_halign(gtk4::Align::End);
        } else {
            input_widget.set_hexpand(true);
            input_widget.set_halign(gtk4::Align::Fill);
        }
        input_box.append(&input_widget);

        self.input_size_group.add_widget(&input_box);
        self.grid.attach(&input_box, input_col, row, input_width, 1);
        row_widgets.push(input_box.upcast());

        self.next_row.set(row + 1);
        row_widgets
    }

    pub fn widget(&self) -> &gtk4::Grid {
        &self.grid
    }
}
