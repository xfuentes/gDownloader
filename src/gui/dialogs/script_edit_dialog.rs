use adw::prelude::*;
use gtk4::glib;
use sourceview5::prelude::*;
use std::rc::Rc;

use crate::gui::components::{ConfigSection, ConfigSectionForm};
use crate::gui::fields::ellipsize_dropdown;
use crate::jd::{EventTrigger, ScriptEntry};

pub struct ScriptEditDialog;

impl ScriptEditDialog {
    pub fn show(parent: &gtk4::Window, entry: ScriptEntry, on_save: Rc<dyn Fn(ScriptEntry)>) {
        let dialog = gtk4::Window::new();
        dialog.set_transient_for(Some(parent));
        dialog.set_modal(true);
        dialog.set_resizable(true);
        dialog.set_default_size(720, 640);
        dialog.set_title(Some(tr!("Edit Script").as_ref()));

        let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        outer.set_vexpand(true);
        outer.set_hexpand(true);

        let page_box = gtk4::Box::new(gtk4::Orientation::Vertical, 15);
        page_box.set_margin_top(15);
        page_box.set_margin_bottom(15);
        page_box.set_margin_start(15);
        page_box.set_margin_end(15);
        page_box.set_vexpand(true);

        let label_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let checkbox_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let input_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        let form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let name_entry = gtk4::Entry::new();
        name_entry.set_hexpand(true);
        form.add_row(tr!("Name").as_ref(), None, &name_entry);

        let trigger_model = gtk4::StringList::new(&[]);
        for t in EventTrigger::ALL {
            trigger_model.append(&t.label());
        }
        let trigger_dropdown = ellipsize_dropdown(trigger_model);
        form.add_row(tr!("Event trigger").as_ref(), None, &trigger_dropdown);

        // JDownloader only ever *shows* these two rows when the selected
        // trigger actually supports them (`EventTrigger.createSettingsPanel`
        // only adds the synchronous checkbox when `isSynchronousSupported()`,
        // and only the INTERVAL trigger's override adds the interval
        // spinner at all) — it doesn't just gray them out. `add_row`
        // returns each row's widgets so we can hide the whole row the same
        // way (a GtkGrid row with every widget invisible collapses to zero
        // height on its own).
        let sync_switch = gtk4::Switch::new();
        let sync_row = form.add_row(tr!("Run synchronously").as_ref(), None, &sync_switch);

        let interval_spin = gtk4::SpinButton::new(
            Some(&gtk4::Adjustment::new(1000.0, 100.0, 3_600_000.0, 100.0, 1000.0, 0.0)),
            1.0,
            0,
        );
        let interval_row = form.add_row(tr!("Interval (ms)").as_ref(), None, &interval_spin);

        let section = ConfigSection::new(
            crate::gui::icon_key::ICON_EVENT,
            tr!("Trigger").as_ref(),
            Some(tr!("Choose when this script should run.").as_ref()),
            &form,
        );
        page_box.append(section.widget());

        let script_label = gtk4::Label::new(Some(tr!("Script (JavaScript)").as_ref()));
        script_label.set_halign(gtk4::Align::Start);
        script_label.add_css_class("heading");
        page_box.append(&script_label);

        let buffer = sourceview5::Buffer::new(None);
        if let Some(lang) = sourceview5::LanguageManager::default().language("js") {
            buffer.set_language(Some(&lang));
        }
        buffer.set_highlight_syntax(true);
        let scheme_id = if adw::StyleManager::default().is_dark() {
            "Adwaita-dark"
        } else {
            "Adwaita"
        };
        if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme(scheme_id) {
            buffer.set_style_scheme(Some(&scheme));
        }

        let text_view = sourceview5::View::with_buffer(&buffer);
        text_view.set_monospace(true);
        text_view.set_show_line_numbers(true);
        text_view.set_top_margin(6);
        text_view.set_bottom_margin(6);
        text_view.set_left_margin(6);
        text_view.set_right_margin(6);

        let script_scroll = gtk4::ScrolledWindow::builder()
            .child(&text_view)
            .vexpand(true)
            .hexpand(true)
            .min_content_height(220)
            .propagate_natural_height(true)
            .build();
        script_scroll.add_css_class("card");
        page_box.append(&script_scroll);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page_box));
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);
        outer.append(&scrolled);

        let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());
        let save_btn = gtk4::Button::with_label(tr!("Save").as_ref());
        save_btn.add_css_class("suggested-action");

        let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        button_size_group.add_widget(&cancel_btn);
        button_size_group.add_widget(&save_btn);

        let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        button_box.set_halign(gtk4::Align::End);
        button_box.set_margin_top(6);
        button_box.set_margin_bottom(12);
        button_box.set_margin_start(15);
        button_box.set_margin_end(15);
        button_box.append(&save_btn);
        button_box.append(&cancel_btn);
        outer.append(&button_box);

        dialog.set_child(Some(&outer));
        dialog.set_default_widget(Some(&save_btn));

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

        // Populate from the current entry.
        name_entry.set_text(entry.name.as_deref().unwrap_or(""));
        buffer.set_text(entry.script.as_deref().unwrap_or(""));
        if let Some(idx) = EventTrigger::ALL.iter().position(|t| *t == entry.event_trigger) {
            trigger_dropdown.set_selected(idx as u32);
        }
        sync_switch.set_active(entry.is_synchronous());
        interval_spin.set_value(entry.interval_ms() as f64);

        fn set_row_visible(row: &[gtk4::Widget], visible: bool) {
            for w in row {
                w.set_visible(visible);
            }
        }

        fn update_trigger_row_visibility(
            trigger: EventTrigger,
            sync_row: &[gtk4::Widget],
            interval_row: &[gtk4::Widget],
        ) {
            set_row_visible(sync_row, trigger.supports_synchronous());
            set_row_visible(interval_row, trigger.is_interval());
        }
        update_trigger_row_visibility(entry.event_trigger, &sync_row, &interval_row);

        let sync_row_c = sync_row.clone();
        let interval_row_c = interval_row.clone();
        trigger_dropdown.connect_selected_notify(move |d| {
            let idx = d.selected() as usize;
            if let Some(trigger) = EventTrigger::ALL.get(idx).copied() {
                update_trigger_row_visibility(trigger, &sync_row_c, &interval_row_c);
            }
        });

        cancel_btn.connect_clicked({
            let dialog = dialog.clone();
            move |_| dialog.close()
        });

        let dialog_c = dialog.clone();
        save_btn.connect_clicked(move |_| {
            let mut e = entry.clone();
            e.name = Some(name_entry.text().to_string());
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            e.script = Some(text.to_string());
            let idx = trigger_dropdown.selected() as usize;
            e.event_trigger = EventTrigger::ALL.get(idx).copied().unwrap_or_default();
            e.set_synchronous(sync_switch.is_active());
            if e.event_trigger.is_interval() {
                e.set_interval_ms(interval_spin.value() as i64);
            }

            on_save(e);
            dialog_c.close();
        });

        dialog.present();
        save_btn.grab_focus();
    }
}
