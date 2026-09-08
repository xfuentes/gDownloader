use adw::prelude::*;

/// Injects the CSS backing this cell's `.download-progress-*` classes.
/// Call once during app startup (from `window::setup_css`).
pub fn install_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_string(
        ".download-progress-cell { padding: 0; margin: 0; }\n\
         .download-progress-bar trough { padding: 0;  }\n\
         .download-progress-bar, .download-progress-bar trough, .download-progress-bar progress {\n\
             border-radius: 2px;\n\
             margin: 0;\n\
         }\n\
         .download-progress-bar, .download-progress-bar trough, .download-progress-bar progress { min-height: 20px; }\n\
         .download-progress-bar trough {\n\
             background-image: linear-gradient(to bottom,\n\
                 mix(@theme_bg_color, white, 0.65) 0%,\n\
                 @theme_bg_color 30%,\n\
                 mix(@theme_bg_color, white, 0.65) 100%);\n\
             background-color: @theme_bg_color;\n\
             border: 1px solid @borders;\n\
         }\n\
         .download-progress-bar progress {\n\
             background-image: linear-gradient(to bottom,\n\
                 mix(@accent_bg_color, white, 0.65) 0%,\n\
                 @accent_bg_color 30%,\n\
                 mix(@accent_bg_color, white, 0.65) 100%);\n\
             background-color: @accent_bg_color;\n\
         }\n\
         .download-progress-label { color: @theme_fg_color; font-weight: normal; }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Builds a `GtkColumnViewColumn` that renders a progress fraction as a
/// `GtkOverlay` of a `GtkProgressBar` (stretched to fill the whole cell,
/// its native `show-text` left off) with a percentage label overlaid and
/// centered on top. `GtkColumnView` has no equivalent of
/// `GtkCellRendererProgress` — whose value + text are painted together
/// over the whole cell in `GtkTreeView` — so that look is rebuilt here
/// from real widgets instead.
///
/// `fraction_of` computes the 0.0-1.0 fill fraction for a bound list item,
/// returning `None` to leave a row unbound (e.g. a row kind the column
/// doesn't apply to).
pub fn build_column(
    title: &str,
    width: i32,
    resizable: bool,
    fraction_of: impl Fn(&gtk4::ListItem) -> Option<f64> + 'static,
) -> gtk4::ColumnViewColumn {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
        let overlay = gtk4::Overlay::new();
        overlay.set_halign(gtk4::Align::Fill);
        overlay.set_valign(gtk4::Align::Fill);
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);

        let bar = gtk4::ProgressBar::new();
        bar.set_show_text(false);
        bar.add_css_class("download-progress-bar");
        bar.set_halign(gtk4::Align::Fill);
        bar.set_valign(gtk4::Align::Fill);
        bar.set_hexpand(true);
        bar.set_vexpand(true);
        overlay.set_child(Some(&bar));

        let label = gtk4::Label::new(None);
        label.add_css_class("download-progress-label");
        label.set_halign(gtk4::Align::Center);
        label.set_valign(gtk4::Align::Center);
        // Centering with valign::Center splits margin-top/bottom evenly
        // around the natural position, so a bottom-only margin of `n`
        // shifts the label up by `n / 2`: 2px here for a net 1px rise.
        label.set_margin_bottom(2);
        overlay.add_overlay(&label);

        list_item.set_child(Some(&overlay));
        // GtkColumnView's internal per-cell widget pads its content by
        // default, which keeps the bar from touching the row's top and
        // bottom edges. It isn't exposed by the public API, so reach for
        // it via the overlay's parent and strip that padding with a
        // dedicated CSS class instead.
        if let Some(cell) = overlay.parent() {
            cell.add_css_class("download-progress-cell");
        }
    });
    factory.connect_bind(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
        let Some(fraction) = fraction_of(list_item) else {
            return;
        };
        let Some(overlay) = list_item.child().and_downcast::<gtk4::Overlay>() else {
            return;
        };
        let Some(bar) = overlay.first_child().and_downcast::<gtk4::ProgressBar>() else {
            return;
        };
        let Some(label) = bar.next_sibling().and_downcast::<gtk4::Label>() else {
            return;
        };
        bar.set_fraction(fraction);
        label.set_text(&format!("{:.1}%", fraction * 100.0));
    });
    let col = gtk4::ColumnViewColumn::new(Some(title), Some(factory));
    col.set_fixed_width(width);
    col.set_resizable(resizable);
    col
}
