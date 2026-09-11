use gtk4::glib;
use gtk4::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use crate::gui::dialogs::WaitDialog;
use crate::gui::fields::icon_dropdown;
use crate::gui::icon_key;
use crate::gui::properties_panel::{priority_icons, priority_label, PRIORITIES};
use crate::gui::spawn;
use crate::jd::{AddLinksOptions, GeneralSettings, JdApi, LinkgrabberSettings};

/// Adds an icon + tooltip in front of `content`, mirroring how JDownloader's
/// own "Add Links" dialog labels each field (an icon with a help tooltip,
/// no separate text label).
fn add_field_row(container: &gtk4::Box, icon: &str, tooltip: &str, content: &gtk4::Widget) {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.set_margin_top(4);
    row.set_margin_bottom(4);
    let icon_image = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(icon));
    icon_image.set_pixel_size(20);
    icon_image.set_valign(gtk4::Align::Center);
    icon_image.set_tooltip_text(Some(tooltip));
    row.append(&icon_image);
    row.append(content);
    container.append(&row);
}

pub struct AddLinksDialog;

impl AddLinksDialog {
    pub fn show(parent: &gtk4::Window, api: &Arc<JdApi>) {
        let dialog = gtk4::Window::new();
        dialog.set_transient_for(Some(parent));
        dialog.set_modal(true);
        dialog.set_resizable(true);
        dialog.set_default_size(700, 560);
        dialog.set_title(Some(tr!("Analyse and Add Links").as_ref()));

        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        vbox.set_margin_top(18);
        vbox.set_margin_bottom(18);
        vbox.set_margin_start(18);
        vbox.set_margin_end(18);

        // Links text area, with a JD-style placeholder hint.
        let links_view = gtk4::TextView::new();
        links_view.set_wrap_mode(gtk4::WrapMode::WordChar);
        links_view.set_top_margin(8);
        links_view.set_bottom_margin(8);
        links_view.set_left_margin(8);
        links_view.set_right_margin(8);
        let links_buffer = links_view.buffer();

        let links_scroll = gtk4::ScrolledWindow::builder()
            .child(&links_view)
            .vexpand(true)
            .hexpand(true)
            .build();

        let links_placeholder = gtk4::Label::new(Some(
            tr!("Enter Links, URLs, Websites, or any other text here...").as_ref(),
        ));
        links_placeholder.set_halign(gtk4::Align::Start);
        links_placeholder.set_valign(gtk4::Align::Start);
        links_placeholder.set_margin_start(10);
        links_placeholder.set_margin_top(10);
        links_placeholder.add_css_class("dim-label");
        links_placeholder.set_can_target(false);

        let links_overlay = gtk4::Overlay::new();
        links_overlay.set_child(Some(&links_scroll));
        links_overlay.add_overlay(&links_placeholder);
        vbox.append(&links_overlay);

        // Form fields.
        let form_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        let dest_entry = gtk4::Entry::new();
        dest_entry.set_hexpand(true);
        let browse_btn = gtk4::Button::with_label(tr!("Browse").as_ref());
        browse_btn.set_valign(gtk4::Align::Center);
        let dest_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        dest_row.set_hexpand(true);
        dest_row.append(&dest_entry);
        dest_row.append(&browse_btn);
        add_field_row(
            &form_box,
            icon_key::ICON_SAVE,
            &tr!("Please choose Download Destination here."),
            dest_row.upcast_ref::<gtk4::Widget>(),
        );

        let package_entry = gtk4::Entry::new();
        package_entry.set_hexpand(true);
        package_entry
            .set_placeholder_text(Some(tr!("Enter a Package name, or leave empty for auto mode").as_ref()));
        add_field_row(
            &form_box,
            icon_key::ICON_PACKAGE_OPEN,
            &tr!("Choose a Package Name for the Downloads above. If empty, JDownloader will create Packages based on the filenames"),
            package_entry.upcast_ref::<gtk4::Widget>(),
        );

        let comment_entry = gtk4::Entry::new();
        comment_entry.set_hexpand(true);
        comment_entry.set_placeholder_text(Some(tr!("Enter a comment or leave empty...").as_ref()));
        add_field_row(
            &form_box,
            icon_key::ICON_DOCUMENT,
            &tr!("Enter a comment. This comment will be stored in every package and downloadlink."),
            comment_entry.upcast_ref::<gtk4::Widget>(),
        );

        let extract_password_entry = gtk4::Entry::new();
        extract_password_entry.set_hexpand(true);
        extract_password_entry.set_visibility(false);
        extract_password_entry
            .set_placeholder_text(Some(tr!("Enter the archive's extraction password").as_ref()));
        let auto_extract_check = gtk4::CheckButton::with_label(tr!("Auto Extract").as_ref());
        auto_extract_check
            .set_tooltip_text(Some(tr!("Enable this option to extract all found archives after download").as_ref()));
        let extract_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        extract_row.set_hexpand(true);
        extract_row.append(&extract_password_entry);
        extract_row.append(&auto_extract_check);
        add_field_row(
            &form_box,
            icon_key::ICON_EXTRACT,
            &tr!("Enter the archive's extraction password"),
            extract_row.upcast_ref::<gtk4::Widget>(),
        );

        let download_password_entry = gtk4::Entry::new();
        download_password_entry.set_hexpand(true);
        download_password_entry.set_visibility(false);
        download_password_entry
            .set_placeholder_text(Some(tr!("Enter a Password for protected Links").as_ref()));
        let priority_model = gtk4::StringList::new(&[]);
        for key in PRIORITIES {
            priority_model.append(&priority_label(key));
        }
        let priority_dropdown = icon_dropdown(priority_model, priority_icons());
        priority_dropdown.set_selected(3); // DEFAULT
        let password_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        password_row.set_hexpand(true);
        download_password_entry.set_hexpand(true);
        priority_dropdown.set_hexpand(false);
        password_row.append(&download_password_entry);
        password_row.append(&priority_dropdown);
        add_field_row(
            &form_box,
            icon_key::ICON_PASSWORD,
            &tr!("Enter a Password for protected Links"),
            password_row.upcast_ref::<gtk4::Widget>(),
        );

        vbox.append(&form_box);

        // "Information overwrites packagizer rules" — auto-toggles the
        // moment a custom field diverges from its loaded default, unless
        // the user already flipped it by hand.
        let overwrite_check =
            gtk4::CheckButton::with_label(tr!("Information overwrites packagizer rules").as_ref());
        let overwrite_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let overwrite_icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(icon_key::ICON_UPLOAD));
        overwrite_icon.set_pixel_size(20);
        overwrite_box.append(&overwrite_icon);
        overwrite_box.append(&overwrite_check);

        let touched_overwrite = Rc::new(Cell::new(false));
        let programmatic_overwrite = Rc::new(Cell::new(false));
        let default_auto_extract = Rc::new(Cell::new(true));

        overwrite_check.connect_toggled({
            let touched = touched_overwrite.clone();
            let programmatic = programmatic_overwrite.clone();
            move |_| {
                if !programmatic.get() {
                    touched.set(true);
                }
            }
        });

        let recheck_overwrite: Rc<dyn Fn()> = {
            let package_entry = package_entry.clone();
            let comment_entry = comment_entry.clone();
            let extract_password_entry = extract_password_entry.clone();
            let download_password_entry = download_password_entry.clone();
            let priority_dropdown = priority_dropdown.clone();
            let auto_extract_check = auto_extract_check.clone();
            let overwrite_check = overwrite_check.clone();
            let default_auto_extract = default_auto_extract.clone();
            let touched_overwrite = touched_overwrite.clone();
            let programmatic_overwrite = programmatic_overwrite.clone();
            Rc::new(move || {
                if touched_overwrite.get() {
                    return;
                }
                let differs = !package_entry.text().is_empty()
                    || !comment_entry.text().is_empty()
                    || !extract_password_entry.text().is_empty()
                    || !download_password_entry.text().is_empty()
                    || priority_dropdown.selected() != 3
                    || auto_extract_check.is_active() != default_auto_extract.get();
                if differs != overwrite_check.is_active() {
                    programmatic_overwrite.set(true);
                    overwrite_check.set_active(differs);
                    programmatic_overwrite.set(false);
                }
            })
        };
        for entry in [&package_entry, &comment_entry, &extract_password_entry, &download_password_entry] {
            entry.connect_changed({
                let recheck = recheck_overwrite.clone();
                move |_| recheck()
            });
        }
        priority_dropdown.connect_selected_notify({
            let recheck = recheck_overwrite.clone();
            move |_| recheck()
        });
        auto_extract_check.connect_toggled({
            let recheck = recheck_overwrite.clone();
            move |_| recheck()
        });

        // Bottom row: the "overwrites packagizer rules" checkbox on the
        // left, Cancel and Continue (+ its Deep/Normal Analyse arrow menu,
        // visually joined via GTK's "linked" segmented style) on the right.
        let continue_btn = gtk4::Button::with_label(tr!("Continue").as_ref());
        continue_btn.set_sensitive(false);
        continue_btn.add_css_class("suggested-action");

        let deep_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        deep_popover_box.set_margin_start(8);
        deep_popover_box.set_margin_end(8);
        deep_popover_box.set_margin_top(8);
        deep_popover_box.set_margin_bottom(8);
        let deep_row = gtk4::Button::builder()
            .label(tr!("Start Deep Link Analyse").as_str())
            .has_frame(false)
            .halign(gtk4::Align::Start)
            .build();
        let normal_row = gtk4::Button::builder()
            .label(tr!("Start Normal Link Analyse").as_str())
            .has_frame(false)
            .halign(gtk4::Align::Start)
            .build();
        deep_popover_box.append(&deep_row);
        deep_popover_box.append(&normal_row);
        let deep_popover = gtk4::Popover::new();
        deep_popover.set_child(Some(&deep_popover_box));
        let deep_arrow = gtk4::MenuButton::new();
        deep_arrow.set_direction(gtk4::ArrowType::Down);
        deep_arrow.set_popover(Some(&deep_popover));

        let continue_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        continue_box.add_css_class("linked");
        continue_box.append(&continue_btn);
        continue_box.append(&deep_arrow);

        let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());

        // Every action button in this dialog shares one width (memory rule).
        let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Both);
        button_size_group.add_widget(&continue_btn);
        button_size_group.add_widget(&cancel_btn);

        let button_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        button_row.append(&overwrite_box);
        let button_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        button_spacer.set_hexpand(true);
        button_row.append(&button_spacer);
        button_row.append(&continue_box);
        button_row.append(&cancel_btn);
        vbox.append(&button_row);

        cancel_btn.connect_clicked({
            let dialog = dialog.clone();
            move |_| dialog.close()
        });

        dialog.set_child(Some(&vbox));
        dialog.set_default_widget(Some(&continue_btn));

        links_buffer.connect_changed({
            let placeholder = links_placeholder.clone();
            let continue_btn = continue_btn.clone();
            move |buf| {
                let empty = buf.char_count() == 0;
                placeholder.set_visible(empty);
                continue_btn.set_sensitive(!empty);
            }
        });

        let escape_controller = gtk4::EventControllerKey::new();
        escape_controller.connect_key_pressed({
            let dialog = dialog.clone();
            move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    dialog.close();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        });
        dialog.add_controller(escape_controller);

        // Browse for a destination folder.
        browse_btn.connect_clicked({
            let dest_entry = dest_entry.clone();
            let dialog = dialog.clone();
            move |_| {
                let file_dialog = gtk4::FileDialog::new();
                file_dialog.set_title(tr!("Select Folder").as_ref());
                let dest_entry = dest_entry.clone();
                glib::MainContext::default().spawn_local({
                    let dialog = dialog.clone();
                    async move {
                        match file_dialog.select_folder_future(Some(&dialog)).await {
                            Ok(file) => {
                                if let Some(path) = file.path() {
                                    dest_entry.set_text(&path.to_string_lossy());
                                }
                            }
                            Err(e) => log::warn!("Failed to select folder: {}", e),
                        }
                    }
                });
            }
        });

        // Pre-fill destination/checkboxes from JDownloader's LinkgrabberSettings,
        // and the links text area from the clipboard, mirroring JD's own dialog.
        spawn::api_call(
            {
                let api = Arc::clone(api);
                move || {
                    let settings = LinkgrabberSettings::new(api.clone());
                    let general = GeneralSettings::new(api.clone());
                    // Always the configured default download folder — unlike
                    // JD's own dialog, this doesn't fall back to the last
                    // folder used, which could otherwise surprise the user
                    // with a folder they don't recognize as "the default".
                    let folder = general.get_default_download_folder().unwrap_or_default();
                    let auto_extract = settings.get_auto_extraction_enabled().unwrap_or(true);
                    let overwrite_default = settings
                        .get_add_links_dialog_overwrites_packagizer_rules_enabled()
                        .unwrap_or(true);
                    let auto_fill_clipboard = settings
                        .get_auto_fill_add_links_dialog_with_clipboard_content_enabled()
                        .unwrap_or(true);
                    (folder, auto_extract, overwrite_default, auto_fill_clipboard)
                }
            },
            {
                let dest_entry = dest_entry.clone();
                let auto_extract_check = auto_extract_check.clone();
                let overwrite_check = overwrite_check.clone();
                let default_auto_extract = default_auto_extract.clone();
                let programmatic_overwrite = programmatic_overwrite.clone();
                let links_buffer = links_buffer.clone();
                move |(folder, auto_extract, overwrite_default, auto_fill_clipboard)| {
                    dest_entry.set_text(&folder);
                    default_auto_extract.set(auto_extract);
                    auto_extract_check.set_active(auto_extract);
                    programmatic_overwrite.set(true);
                    overwrite_check.set_active(overwrite_default);
                    programmatic_overwrite.set(false);
                    if auto_fill_clipboard {
                        if let Some(display) = gtk4::gdk::Display::default() {
                            let links_buffer = links_buffer.clone();
                            display.clipboard().read_text_async(gtk4::gio::Cancellable::NONE, move |res| {
                                if let Ok(Some(text)) = res {
                                    if links_buffer.char_count() == 0 && !text.trim().is_empty() {
                                        links_buffer.set_text(&text);
                                    }
                                }
                            });
                        }
                    }
                }
            },
        );

        // Submit: builds `AddLinksOptions` from the current field values and
        // hands them to `linkgrabberv2/addLinks`.
        let submit: Rc<dyn Fn(bool)> = {
            let dialog = dialog.clone();
            let api = Arc::clone(api);
            let links_buffer = links_buffer.clone();
            let dest_entry = dest_entry.clone();
            let package_entry = package_entry.clone();
            let comment_entry = comment_entry.clone();
            let extract_password_entry = extract_password_entry.clone();
            let download_password_entry = download_password_entry.clone();
            let auto_extract_check = auto_extract_check.clone();
            let overwrite_check = overwrite_check.clone();
            let priority_dropdown = priority_dropdown.clone();
            Rc::new(move |deep_decrypt: bool| {
                let start = links_buffer.start_iter();
                let end = links_buffer.end_iter();
                let links_text = links_buffer.text(&start, &end, true).to_string();
                if links_text.trim().is_empty() {
                    return;
                }
                let priority = PRIORITIES[priority_dropdown.selected() as usize].to_string();
                let opts = AddLinksOptions {
                    links: links_text,
                    autostart: false,
                    package_name: package_entry.text().to_string(),
                    extract_password: extract_password_entry.text().to_string(),
                    download_password: download_password_entry.text().to_string(),
                    destination_folder: dest_entry.text().to_string(),
                    comment: comment_entry.text().to_string(),
                    auto_extract: auto_extract_check.is_active(),
                    deep_decrypt,
                    overwrite_packagizer_rules: overwrite_check.is_active(),
                    priority,
                };

                let wait = WaitDialog::show(&dialog, tr!("Adding links...").as_ref());
                let dialog = dialog.clone();
                let api = Arc::clone(&api);
                spawn::api_call(
                    move || api.add_links_with_options(&opts),
                    move |result| {
                        wait.close();
                        if let Err(e) = result {
                            log::warn!("add_links_with_options failed: {}", e);
                        }
                        dialog.close();
                    },
                );
            })
        };

        continue_btn.connect_clicked({
            let submit = submit.clone();
            move |_| submit(false)
        });
        deep_row.connect_clicked({
            let submit = submit.clone();
            let deep_popover = deep_popover.clone();
            move |_| {
                deep_popover.popdown();
                submit(true);
            }
        });
        normal_row.connect_clicked({
            let submit = submit.clone();
            let deep_popover = deep_popover.clone();
            move |_| {
                deep_popover.popdown();
                submit(false);
            }
        });

        dialog.present();
        links_view.grab_focus();
    }
}
