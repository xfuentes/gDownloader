use adw::prelude::*;
use gtk4::glib;
use std::rc::Rc;

use crate::gui::components::{ConfigSection, ConfigSectionForm};
use crate::gui::fields::ellipsize_dropdown;
use crate::jd::{
    BooleanFilter, FilesizeFilter, FiletypeFilter, PackagizerRule, Priority, RegexFilter,
    RegexMatchType, SizeMatchType, TypeMatchType,
};

const REGEX_MATCH_TAGS: [RegexMatchType; 4] = [
    RegexMatchType::Contains,
    RegexMatchType::Equals,
    RegexMatchType::ContainsNot,
    RegexMatchType::EqualsNot,
];

fn regex_match_type_labels() -> gtk4::StringList {
    let model = gtk4::StringList::new(&[]);
    model.append(tr!("Contains").as_ref());
    model.append(tr!("Equals").as_ref());
    model.append(tr!("Does not contain").as_ref());
    model.append(tr!("Does not equal").as_ref());
    model
}

/// A single `RegexFilter` condition row: an enable switch, a match-type
/// dropdown, a text/regex entry and a "use regex" checkbox.
struct RegexFilterRow {
    enable: gtk4::Switch,
    match_type: gtk4::DropDown,
    entry: gtk4::Entry,
    use_regex: gtk4::CheckButton,
}

impl RegexFilterRow {
    fn build(form: &ConfigSectionForm, label: &str) -> Self {
        let enable = gtk4::Switch::new();

        let match_type = ellipsize_dropdown(regex_match_type_labels());
        match_type.set_sensitive(false);

        let entry = gtk4::Entry::new();
        entry.set_hexpand(true);
        entry.set_sensitive(false);

        let use_regex = gtk4::CheckButton::with_label(tr!("Regex").as_ref());
        use_regex.set_sensitive(false);

        let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        row_box.set_hexpand(true);
        row_box.append(&match_type);
        row_box.append(&entry);
        row_box.append(&use_regex);

        form.add_row(label, Some(&enable), &row_box);

        let match_type_c = match_type.clone();
        let entry_c = entry.clone();
        let use_regex_c = use_regex.clone();
        enable.connect_state_notify(move |s| {
            let active = s.is_active();
            match_type_c.set_sensitive(active);
            entry_c.set_sensitive(active);
            use_regex_c.set_sensitive(active);
        });

        Self { enable, match_type, entry, use_regex }
    }

    fn set(&self, f: &RegexFilter) {
        self.enable.set_active(f.enabled);
        self.match_type.set_sensitive(f.enabled);
        self.entry.set_sensitive(f.enabled);
        self.use_regex.set_sensitive(f.enabled);
        if let Some(idx) = REGEX_MATCH_TAGS.iter().position(|t| *t == f.match_type) {
            self.match_type.set_selected(idx as u32);
        }
        self.entry.set_text(f.regex.as_deref().unwrap_or(""));
        self.use_regex.set_active(f.use_regex);
    }

    fn get(&self) -> RegexFilter {
        let idx = self.match_type.selected() as usize;
        RegexFilter {
            enabled: self.enable.is_active(),
            match_type: REGEX_MATCH_TAGS.get(idx).copied().unwrap_or_default(),
            regex: Some(self.entry.text().to_string()),
            use_regex: self.use_regex.is_active(),
        }
    }
}

/// An action row that is either unset or one of two states ("Enabled" /
/// "Disabled"), matching JDownloader's `Boolean`-typed action fields.
struct EnabledDisabledRow {
    enable: gtk4::Switch,
    dropdown: gtk4::DropDown,
}

impl EnabledDisabledRow {
    fn build(form: &ConfigSectionForm, label: &str) -> Self {
        let enable = gtk4::Switch::new();
        let model = gtk4::StringList::new(&[]);
        model.append(tr!("Enabled").as_ref());
        model.append(tr!("Disabled").as_ref());
        let dropdown = ellipsize_dropdown(model);
        dropdown.set_sensitive(false);

        form.add_row(label, Some(&enable), &dropdown);

        let dropdown_c = dropdown.clone();
        enable.connect_state_notify(move |s| dropdown_c.set_sensitive(s.is_active()));

        Self { enable, dropdown }
    }

    fn set(&self, value: Option<bool>) {
        let active = value.is_some();
        self.enable.set_active(active);
        self.dropdown.set_sensitive(active);
        self.dropdown.set_selected(if value.unwrap_or(true) { 0 } else { 1 });
    }

    fn get(&self) -> Option<bool> {
        if self.enable.is_active() {
            Some(self.dropdown.selected() == 0)
        } else {
            None
        }
    }
}

/// A checkbox-gated free-text action row (destination folder, package name,
/// comment, rename pattern, ...).
struct TextActionRow {
    enable: gtk4::Switch,
    entry: gtk4::Entry,
}

impl TextActionRow {
    fn build(form: &ConfigSectionForm, label: &str, browse: bool) -> Self {
        let enable = gtk4::Switch::new();
        let entry = gtk4::Entry::new();
        entry.set_hexpand(true);
        entry.set_sensitive(false);

        let widget: gtk4::Box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        widget.set_hexpand(true);
        widget.append(&entry);

        if browse {
            let browse_btn = gtk4::Button::with_label(tr!("Browse").as_ref());
            browse_btn.set_sensitive(false);
            widget.append(&browse_btn);

            let entry_for_browse = entry.clone();
            browse_btn.connect_clicked(move |btn| {
                let dialog = gtk4::FileDialog::new();
                dialog.set_title(tr!("Select Folder").as_ref());
                let entry_for_browse = entry_for_browse.clone();
                let root = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
                glib::MainContext::default().spawn_local(async move {
                    if let Ok(file) = dialog.select_folder_future(root.as_ref()).await {
                        if let Some(path) = file.path() {
                            entry_for_browse.set_text(&path.to_string_lossy());
                        }
                    }
                });
            });

            let browse_btn_c = browse_btn.clone();
            enable.connect_state_notify(move |s| browse_btn_c.set_sensitive(s.is_active()));
        }

        form.add_row(label, Some(&enable), &widget);

        let entry_c = entry.clone();
        enable.connect_state_notify(move |s| entry_c.set_sensitive(s.is_active()));

        Self { enable, entry }
    }

    fn set(&self, value: &Option<String>) {
        let active = value.as_deref().is_some_and(|s| !s.is_empty());
        self.enable.set_active(active);
        self.entry.set_sensitive(active);
        self.entry.set_text(value.as_deref().unwrap_or(""));
    }

    fn get(&self) -> Option<String> {
        if self.enable.is_active() {
            Some(self.entry.text().to_string())
        } else {
            None
        }
    }
}

pub struct PackagizerRuleDialog;

impl PackagizerRuleDialog {
    pub fn show(parent: &gtk4::Window, rule: PackagizerRule, on_save: Rc<dyn Fn(PackagizerRule)>) {
        let dialog = gtk4::Window::new();
        dialog.set_transient_for(Some(parent));
        dialog.set_modal(true);
        dialog.set_resizable(true);
        dialog.set_default_size(720, 760);
        dialog.set_title(Some(tr!("Package Manager Rule").as_ref()));

        let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        outer.set_vexpand(true);
        outer.set_hexpand(true);

        let page_box = gtk4::Box::new(gtk4::Orientation::Vertical, 15);
        page_box.set_margin_top(15);
        page_box.set_margin_bottom(15);
        page_box.set_margin_start(15);
        page_box.set_margin_end(15);

        let label_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let checkbox_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let input_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        // Name
        let name_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);
        let name_entry = gtk4::Entry::new();
        name_entry.set_hexpand(true);
        name_form.add_row(tr!("Rule name").as_ref(), None, &name_entry);
        page_box.append(name_form.widget());

        // Conditions ("If")
        let cond_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let always_switch = gtk4::Switch::new();
        cond_form.add_row(tr!("Always matches").as_ref(), None, &always_switch);

        let filename_row = RegexFilterRow::build(&cond_form, tr!("Filename").as_ref());
        let packagename_row = RegexFilterRow::build(&cond_form, tr!("Package name").as_ref());
        let comment_row = RegexFilterRow::build(&cond_form, tr!("Comment").as_ref());
        let hoster_row = RegexFilterRow::build(&cond_form, tr!("Hoster / URL").as_ref());
        let source_row = RegexFilterRow::build(&cond_form, tr!("Source URL").as_ref());

        // File size condition
        let size_enable = gtk4::Switch::new();
        let size_match_model = gtk4::StringList::new(&[]);
        size_match_model.append(tr!("Between").as_ref());
        size_match_model.append(tr!("Not between").as_ref());
        let size_match = ellipsize_dropdown(size_match_model);
        size_match.set_sensitive(false);
        let size_from = gtk4::SpinButton::new(
            Some(&gtk4::Adjustment::new(0.0, 0.0, 1_000_000.0, 1.0, 100.0, 0.0)),
            1.0,
            2,
        );
        size_from.set_sensitive(false);
        let size_to = gtk4::SpinButton::new(
            Some(&gtk4::Adjustment::new(0.0, 0.0, 1_000_000.0, 1.0, 100.0, 0.0)),
            1.0,
            2,
        );
        size_to.set_sensitive(false);
        let size_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        size_box.set_hexpand(true);
        size_box.append(&size_match);
        size_box.append(&size_from);
        size_box.append(&gtk4::Label::new(Some(&format!("{} —", tr!("MiB")))));
        size_box.append(&size_to);
        size_box.append(&gtk4::Label::new(Some(tr!("MiB").as_ref())));
        cond_form.add_row(tr!("File size").as_ref(), Some(&size_enable), &size_box);
        {
            let size_match_c = size_match.clone();
            let size_from_c = size_from.clone();
            let size_to_c = size_to.clone();
            size_enable.connect_state_notify(move |s| {
                let active = s.is_active();
                size_match_c.set_sensitive(active);
                size_from_c.set_sensitive(active);
                size_to_c.set_sensitive(active);
            });
        }

        // File type condition
        let type_enable = gtk4::Switch::new();
        let type_match_model = gtk4::StringList::new(&[]);
        type_match_model.append(tr!("Is").as_ref());
        type_match_model.append(tr!("Is not").as_ref());
        let type_match = ellipsize_dropdown(type_match_model);
        type_match.set_sensitive(false);
        let type_video = gtk4::CheckButton::with_label(tr!("Video").as_ref());
        let type_audio = gtk4::CheckButton::with_label(tr!("Audio").as_ref());
        let type_archives = gtk4::CheckButton::with_label(tr!("Archives").as_ref());
        let type_images = gtk4::CheckButton::with_label(tr!("Images").as_ref());
        let type_docs = gtk4::CheckButton::with_label(tr!("Documents").as_ref());
        let type_subs = gtk4::CheckButton::with_label(tr!("Subtitles").as_ref());
        let type_exe = gtk4::CheckButton::with_label(tr!("Executables").as_ref());
        let type_hash = gtk4::CheckButton::with_label(tr!("Hash files").as_ref());
        let type_customs = gtk4::Entry::new();
        type_customs.set_hexpand(true);
        type_customs.set_placeholder_text(Some(tr!("Custom extensions (e.g. iso,bin)").as_ref()));

        let type_checks_box = gtk4::FlowBox::new();
        type_checks_box.set_selection_mode(gtk4::SelectionMode::None);
        type_checks_box.set_max_children_per_line(4);
        type_checks_box.set_hexpand(true);
        for c in [
            &type_video,
            &type_audio,
            &type_archives,
            &type_images,
            &type_docs,
            &type_subs,
            &type_exe,
            &type_hash,
        ] {
            type_checks_box.insert(c, -1);
        }

        let type_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        type_box.set_hexpand(true);
        let type_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        type_top.append(&type_match);
        type_top.append(&type_customs);
        type_box.append(&type_top);
        type_box.append(&type_checks_box);
        cond_form.add_row(tr!("File type").as_ref(), Some(&type_enable), &type_box);
        {
            let widgets: Vec<gtk4::Widget> = vec![
                type_match.clone().upcast(),
                type_customs.clone().upcast(),
                type_video.clone().upcast(),
                type_audio.clone().upcast(),
                type_archives.clone().upcast(),
                type_images.clone().upcast(),
                type_docs.clone().upcast(),
                type_subs.clone().upcast(),
                type_exe.clone().upcast(),
                type_hash.clone().upcast(),
            ];
            type_enable.connect_state_notify(move |s| {
                let active = s.is_active();
                for w in &widgets {
                    w.set_sensitive(active);
                }
            });
        }

        let cond_section = ConfigSection::new(
            crate::gui::icon_key::ICON_PACKAGIZER,
            tr!("If").as_ref(),
            Some(tr!("Match downloads based on these conditions.").as_ref()),
            &cond_form,
        );
        page_box.append(cond_section.widget());

        // "Then" actions
        let then_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let dest_row = TextActionRow::build(&then_form, tr!("Destination Folder").as_ref(), true);

        let priority_enable = gtk4::Switch::new();
        let priority_model = gtk4::StringList::new(&[]);
        priority_model.append(tr!("Highest").as_ref());
        priority_model.append(tr!("Higher").as_ref());
        priority_model.append(tr!("High").as_ref());
        priority_model.append(tr!("Default").as_ref());
        priority_model.append(tr!("Low").as_ref());
        priority_model.append(tr!("Lower").as_ref());
        priority_model.append(tr!("Lowest").as_ref());
        let priority_dropdown = ellipsize_dropdown(priority_model);
        priority_dropdown.set_sensitive(false);
        then_form.add_row(tr!("Priority").as_ref(), Some(&priority_enable), &priority_dropdown);
        {
            let priority_dropdown_c = priority_dropdown.clone();
            priority_enable.connect_state_notify(move |s| priority_dropdown_c.set_sensitive(s.is_active()));
        }

        let package_name_row = TextActionRow::build(&then_form, tr!("Package name").as_ref(), false);
        let package_key_row = TextActionRow::build(&then_form, tr!("Package key").as_ref(), false);
        let filename_action_row = TextActionRow::build(&then_form, tr!("Filename").as_ref(), false);
        let comment_action_row = TextActionRow::build(&then_form, tr!("Comment").as_ref(), false);

        let chunks_enable = gtk4::Switch::new();
        let chunks_spin =
            gtk4::SpinButton::new(Some(&gtk4::Adjustment::new(2.0, 1.0, 20.0, 1.0, 1.0, 0.0)), 1.0, 0);
        chunks_spin.set_sensitive(false);
        then_form.add_row(tr!("Chunks").as_ref(), Some(&chunks_enable), &chunks_spin);
        {
            let chunks_spin_c = chunks_spin.clone();
            chunks_enable.connect_state_notify(move |s| chunks_spin_c.set_sensitive(s.is_active()));
        }

        let extract_row = EnabledDisabledRow::build(&then_form, tr!("Auto extract").as_ref());
        let add_row = EnabledDisabledRow::build(&then_form, tr!("Auto add").as_ref());
        let start_row = EnabledDisabledRow::build(&then_form, tr!("Autostart").as_ref());
        let force_row = EnabledDisabledRow::build(&then_form, tr!("Forced start").as_ref());
        let link_enabled_row = EnabledDisabledRow::build(&then_form, tr!("Link enabled").as_ref());

        let then_section = ConfigSection::new(
            crate::gui::icon_key::ICON_DOWNLOADMANAGMENT,
            tr!("Then").as_ref(),
            None,
            &then_form,
        );
        page_box.append(then_section.widget());

        // "...and do" actions
        let do_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);
        let move_row = TextActionRow::build(&do_form, tr!("Move to").as_ref(), true);
        let rename_row = TextActionRow::build(&do_form, tr!("Rename to").as_ref(), false);

        let do_section = ConfigSection::new(
            crate::gui::icon_key::ICON_PACKAGE_OPEN,
            tr!("...and do").as_ref(),
            None,
            &do_form,
        );
        page_box.append(do_section.widget());

        // Stop after this rule
        let stop_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);
        let stop_switch = gtk4::Switch::new();
        stop_form.add_row(
            tr!("Stop processing further rules if this rule matches").as_ref(),
            None,
            &stop_switch,
        );
        page_box.append(stop_form.widget());

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page_box));
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);
        outer.append(&scrolled);

        // Buttons
        let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());
        let save_btn = gtk4::Button::with_label(tr!("Save").as_ref());
        save_btn.add_css_class("suggested-action");

        if rule.static_rule {
            save_btn.set_sensitive(false);
            save_btn.set_tooltip_text(Some(tr!("This is a predefined rule and cannot be modified.").as_ref()));
        }

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

        // Populate from the current rule.
        name_entry.set_text(rule.name.as_deref().unwrap_or(""));
        always_switch.set_active(rule.match_always_filter.enabled);
        filename_row.set(&rule.filename_filter);
        packagename_row.set(&rule.packagename_filter);
        comment_row.set(&rule.comment_filter);
        hoster_row.set(&rule.hoster_url_filter);
        source_row.set(&rule.source_url_filter);

        size_enable.set_active(rule.filesize_filter.enabled);
        size_match.set_sensitive(rule.filesize_filter.enabled);
        size_from.set_sensitive(rule.filesize_filter.enabled);
        size_to.set_sensitive(rule.filesize_filter.enabled);
        size_match.set_selected(match rule.filesize_filter.match_type {
            SizeMatchType::Between => 0,
            SizeMatchType::NotBetween => 1,
        });
        size_from.set_value(rule.filesize_filter.from as f64 / (1024.0 * 1024.0));
        size_to.set_value(rule.filesize_filter.to as f64 / (1024.0 * 1024.0));

        type_enable.set_active(rule.filetype_filter.enabled);
        for w in [
            &type_match.clone().upcast::<gtk4::Widget>(),
            &type_customs.clone().upcast::<gtk4::Widget>(),
            &type_video.clone().upcast::<gtk4::Widget>(),
            &type_audio.clone().upcast::<gtk4::Widget>(),
            &type_archives.clone().upcast::<gtk4::Widget>(),
            &type_images.clone().upcast::<gtk4::Widget>(),
            &type_docs.clone().upcast::<gtk4::Widget>(),
            &type_subs.clone().upcast::<gtk4::Widget>(),
            &type_exe.clone().upcast::<gtk4::Widget>(),
            &type_hash.clone().upcast::<gtk4::Widget>(),
        ] {
            w.set_sensitive(rule.filetype_filter.enabled);
        }
        type_match.set_selected(match rule.filetype_filter.match_type {
            TypeMatchType::Is => 0,
            TypeMatchType::IsNot => 1,
        });
        type_video.set_active(rule.filetype_filter.video_files_enabled);
        type_audio.set_active(rule.filetype_filter.audio_files_enabled);
        type_archives.set_active(rule.filetype_filter.archives_enabled);
        type_images.set_active(rule.filetype_filter.images_enabled);
        type_docs.set_active(rule.filetype_filter.doc_files_enabled);
        type_subs.set_active(rule.filetype_filter.sub_files_enabled);
        type_exe.set_active(rule.filetype_filter.exe_files_enabled);
        type_hash.set_active(rule.filetype_filter.hash_enabled);
        type_customs.set_text(rule.filetype_filter.customs.as_deref().unwrap_or(""));

        dest_row.set(&rule.download_destination);
        priority_enable.set_active(rule.priority.is_some());
        priority_dropdown.set_sensitive(rule.priority.is_some());
        if let Some(idx) = Priority::ALL.iter().position(|p| Some(*p) == rule.priority) {
            priority_dropdown.set_selected(idx as u32);
        } else {
            priority_dropdown.set_selected(3); // DEFAULT
        }
        package_name_row.set(&rule.package_name);
        package_key_row.set(&rule.package_key);
        filename_action_row.set(&rule.filename);
        comment_action_row.set(&rule.comment);

        let chunks_active = rule.chunks > 0;
        chunks_enable.set_active(chunks_active);
        chunks_spin.set_sensitive(chunks_active);
        if chunks_active {
            chunks_spin.set_value(rule.chunks as f64);
        }

        extract_row.set(rule.auto_extraction_enabled);
        add_row.set(rule.auto_add_enabled);
        start_row.set(rule.auto_start_enabled);
        force_row.set(rule.auto_forced_start_enabled);
        link_enabled_row.set(rule.link_enabled);

        move_row.set(&rule.moveto);
        rename_row.set(&rule.rename);

        stop_switch.set_active(rule.stop_after_this_rule);

        cancel_btn.connect_clicked({
            let dialog = dialog.clone();
            move |_| dialog.close()
        });

        let dialog_c = dialog.clone();
        save_btn.connect_clicked(move |_| {
            let mut r = rule.clone();
            r.name = Some(name_entry.text().to_string());
            r.match_always_filter = BooleanFilter { enabled: always_switch.is_active() };
            r.filename_filter = filename_row.get();
            r.packagename_filter = packagename_row.get();
            r.comment_filter = comment_row.get();
            r.hoster_url_filter = hoster_row.get();
            r.source_url_filter = source_row.get();

            r.filesize_filter = FilesizeFilter {
                enabled: size_enable.is_active(),
                match_type: if size_match.selected() == 0 {
                    SizeMatchType::Between
                } else {
                    SizeMatchType::NotBetween
                },
                from: (size_from.value() * 1024.0 * 1024.0) as i64,
                to: (size_to.value() * 1024.0 * 1024.0) as i64,
            };

            r.filetype_filter = FiletypeFilter {
                enabled: type_enable.is_active(),
                match_type: if type_match.selected() == 0 { TypeMatchType::Is } else { TypeMatchType::IsNot },
                use_regex: false,
                hash_enabled: type_hash.is_active(),
                audio_files_enabled: type_audio.is_active(),
                video_files_enabled: type_video.is_active(),
                archives_enabled: type_archives.is_active(),
                images_enabled: type_images.is_active(),
                doc_files_enabled: type_docs.is_active(),
                sub_files_enabled: type_subs.is_active(),
                exe_files_enabled: type_exe.is_active(),
                customs: {
                    let t = type_customs.text().to_string();
                    if t.is_empty() { None } else { Some(t) }
                },
            };

            r.download_destination = dest_row.get();
            r.priority = if priority_enable.is_active() {
                Priority::ALL.get(priority_dropdown.selected() as usize).copied()
            } else {
                None
            };
            r.package_name = package_name_row.get();
            r.package_key = package_key_row.get();
            r.filename = filename_action_row.get();
            r.comment = comment_action_row.get();
            r.chunks = if chunks_enable.is_active() { chunks_spin.value() as i32 } else { -1 };
            r.auto_extraction_enabled = extract_row.get();
            r.auto_add_enabled = add_row.get();
            r.auto_start_enabled = start_row.get();
            r.auto_forced_start_enabled = force_row.get();
            r.link_enabled = link_enabled_row.get();
            r.moveto = move_row.get();
            r.rename = rename_row.get();
            r.stop_after_this_rule = stop_switch.is_active();

            on_save(r);
            dialog_c.close();
        });

        dialog.present();
        save_btn.grab_focus();
    }
}
