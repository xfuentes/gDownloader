use adw::prelude::*;
use gtk4::glib;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::gui::spawn::api_fire;

use crate::gui::components::{ConfigSection, ConfigSectionForm};
use crate::gui::download_limits::{DownloadLimits, DownloadLimitsCache};
use crate::gui::fields::{ellipsize_dropdown, select_from_api};
use crate::jd::{GeneralSettings, JdApi, LinkgrabberSettings, INTERNAL_JD_PORT};

/// Every General setting *except* chunks/parallel downloads/parallel per
/// host, which live in the shared [`DownloadLimitsCache`] instead (also
/// shown in the Downloads list's own Quick Settings menu).
#[derive(Clone, Debug)]
struct GeneralSettingsData {
    download_folder: String,
    cleanup_after_download: String,
    if_file_exists: String,
    auto_start: String,
    countdown: i32,
    countdown_enabled: bool,
    various_package: bool,
    hash_check: bool,
    hash_retry: bool,
    open_container: bool,
}

impl Default for GeneralSettingsData {
    fn default() -> Self {
        Self {
            download_folder: String::from("~/Downloads"),
            cleanup_after_download: String::from("NEVER"),
            if_file_exists: String::from("ASK_FOR_EACH_FILE"),
            auto_start: String::from("ONLY_IF_EXIT_WITH_RUNNING_DOWNLOADS"),
            countdown: 10,
            countdown_enabled: true,
            various_package: true,
            hash_check: true,
            hash_retry: false,
            open_container: false,
        }
    }
}

fn load_settings_from_api(
    general: &GeneralSettings,
    linkgrabber: &LinkgrabberSettings,
) -> Option<GeneralSettingsData> {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if general.is_ready() {
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
    if !general.is_ready() {
        return None;
    }

    let mut d = GeneralSettingsData::default();

    if let Ok(v) = general.get_default_download_folder() {
        d.download_folder = v;
    }
    if let Ok(v) = general.get_cleanup_after_download_action() {
        d.cleanup_after_download = v;
    }
    if let Ok(v) = general.get_if_file_exists_action() {
        d.if_file_exists = v;
    }
    if let Ok(v) = general.get_auto_start_download_option() {
        d.auto_start = v;
    }
    if let Ok(v) = general.get_show_countdown_on_auto_start_downloads() {
        d.countdown_enabled = v;
    }
    if let Ok(v) = general.get_auto_start_countdown_seconds() {
        d.countdown = v;
    }
    if let Ok(v) = general.get_hash_check_enabled() {
        d.hash_check = v;
    }
    if let Ok(v) = general.get_hash_retry_enabled() {
        d.hash_retry = v;
    }
    if let Ok(v) = general.get_auto_open_container_after_download() {
        d.open_container = v;
    }
    if let Ok(v) = linkgrabber.get_various_package_enabled() {
        d.various_package = v;
    }

    Some(d)
}

pub struct GeneralSettingsPage {
    pub widget: gtk4::ScrolledWindow,
}

impl GeneralSettingsPage {
    pub fn build(download_limits: DownloadLimitsCache) -> Self {
        let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
        let general = GeneralSettings::new(Arc::clone(&api));
        let linkgrabber = LinkgrabberSettings::new(Arc::clone(&api));

        let data = GeneralSettingsData::default();

        let page_box = gtk4::Box::new(gtk4::Orientation::Vertical, 15);
        page_box.set_margin_top(15);
        page_box.set_margin_bottom(0);
        page_box.set_margin_start(15);
        page_box.set_margin_end(15);

        let label_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let checkbox_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let input_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        // Download Folder
        let folder_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let download_folder = gtk4::Entry::new();
        download_folder.set_hexpand(true);
        download_folder.set_halign(gtk4::Align::Fill);

        let browse_btn = gtk4::Button::with_label(tr!("Browse").as_ref());
        browse_btn.set_valign(gtk4::Align::Center);

        let folder_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        folder_box.set_hexpand(true);
        folder_box.set_halign(gtk4::Align::Fill);
        folder_box.append(&download_folder);
        folder_box.append(&browse_btn);

        folder_form.add_row(tr!("Download Folder").as_ref(), None, &folder_box);

        let folder_section = ConfigSection::new(
            crate::gui::icon_key::ICON_DOWNLOADPATH,
            tr!("Download Folder").as_ref(),
            Some(
                tr!("Set the default download path here. Changing default path here, affects only new downloads.").as_ref(),
            ),
            &folder_form,
        );
        page_box.append(folder_section.widget());

        // Download Management
        let mgmt_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        // Chunks/parallel downloads/parallel per host are populated from
        // the shared `DownloadLimitsCache` further down, not from `data`.
        let max_sim_adj = gtk4::Adjustment::new(3.0, 1.0, 20.0, 1.0, 10.0, 0.0);
        let max_sim = gtk4::SpinButton::new(Some(&max_sim_adj), 1.0, 0);
        mgmt_form.add_row(tr!("Max. simultaneous Downloads").as_ref(), None, &max_sim);

        let per_host_switch = gtk4::Switch::new();
        let per_host_adj = gtk4::Adjustment::new(1.0, 1.0, 20.0, 1.0, 10.0, 0.0);
        let per_host_spin = gtk4::SpinButton::new(Some(&per_host_adj), 1.0, 0);
        per_host_spin.set_sensitive(false);
        mgmt_form.add_row(
            tr!("Maximum of simultaneous downloads per host").as_ref(),
            Some(&per_host_switch),
            &per_host_spin,
        );

        let max_chunks_adj = gtk4::Adjustment::new(1.0, 1.0, 20.0, 1.0, 10.0, 0.0);
        let max_chunks = gtk4::SpinButton::new(Some(&max_chunks_adj), 1.0, 0);
        mgmt_form.add_row(tr!("Max. Chunks per Download").as_ref(), None, &max_chunks);

        let cleanup_api = [
            "CLEANUP_IMMEDIATELY",
            "CLEANUP_ONCE_AT_STARTUP",
            "CLEANUP_AFTER_PACKAGE_HAS_FINISHED",
            "NEVER",
        ];
        let remove_model = gtk4::StringList::new(&[]);
        remove_model.append(tr!("Immediate").as_ref());
        remove_model.append(tr!("At start").as_ref());
        remove_model.append(tr!("Package ready").as_ref());
        remove_model.append(tr!("Never").as_ref());
        let remove_combo = ellipsize_dropdown(remove_model);
        mgmt_form.add_row(tr!("Remove finished downloads").as_ref(), None, &remove_combo);

        let exists_api = ["OVERWRITE_FILE", "SKIP_FILE", "AUTO_RENAME", "ASK_FOR_EACH_FILE"];
        let exists_model = gtk4::StringList::new(&[]);
        exists_model.append(tr!("Overwrite existing file").as_ref());
        exists_model.append(tr!("Skip file").as_ref());
        exists_model.append(tr!("Auto-rename").as_ref());
        exists_model.append(tr!("Ask for each file").as_ref());
        let exists_combo = ellipsize_dropdown(exists_model);
        mgmt_form.add_row(tr!("If the file already exists").as_ref(), None, &exists_combo);

        let mgmt_section = ConfigSection::new(
            crate::gui::icon_key::ICON_DOWNLOADMANAGMENT,
            tr!("Download Management").as_ref(),
            Some(
                tr!("Connection limits, Download order, Priorities, ... Set up the Download controller details.").as_ref(),
            ),
            &mgmt_form,
        );
        page_box.append(mgmt_section.widget());

        // Autostart Downloads
        let autostart_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let auto_api = ["ALWAYS", "ONLY_IF_EXIT_WITH_RUNNING_DOWNLOADS", "NEVER"];
        let auto_model = gtk4::StringList::new(&[]);
        auto_model.append(tr!("Always").as_ref());
        auto_model.append(tr!("Only if downloads were running at last Session's end").as_ref());
        auto_model.append(tr!("Never").as_ref());
        let auto_combo = ellipsize_dropdown(auto_model);
        autostart_form.add_row(
            tr!("Autostart downloads at application start").as_ref(),
            None,
            &auto_combo,
        );

        let countdown_switch = gtk4::Switch::new();
        let countdown_adj =
            gtk4::Adjustment::new(data.countdown as f64, 0.0, 120.0, 1.0, 10.0, 0.0);
        let countdown_spin = gtk4::SpinButton::new(Some(&countdown_adj), 1.0, 0);
        countdown_spin.set_sensitive(false);
        autostart_form.add_row(
            tr!("Show Countdown (seconds)").as_ref(),
            Some(&countdown_switch),
            &countdown_spin,
        );

        let autostart_section = ConfigSection::new(
            crate::gui::icon_key::ICON_RESUME,
            tr!("Autostart Downloads").as_ref(),
            Some(
                tr!("Choose if, and when JDownloader should start pending downloads without user interaction.").as_ref(),
            ),
            &autostart_form,
        );
        page_box.append(autostart_section.widget());

        // LinkGrabber
        let lg_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let various_package = gtk4::Switch::new();
        lg_form.add_row(
            tr!("Group single files in a Various package").as_ref(),
            None,
            &various_package,
        );

        let lg_section = ConfigSection::new(
            crate::gui::icon_key::ICON_LINKGRABBER,
            tr!("LinkGrabber").as_ref(),
            None,
            &lg_form,
        );
        page_box.append(lg_section.widget());

        // File Writing
        let fw_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let auto_crc = gtk4::Switch::new();
        fw_form.add_row(tr!("SFV / CRC check when possible").as_ref(), None, &auto_crc);

        let restart_crc = gtk4::Switch::new();
        fw_form.add_row(
            tr!("Restart Downloads when SFV/CRC check fails").as_ref(),
            None,
            &restart_crc,
        );

        let fw_section = ConfigSection::new(
            crate::gui::icon_key::ICON_HASHSUM,
            tr!("File Writing").as_ref(),
            Some(
                tr!("JDownloader will verify your downloads for correctness after download using the hash/check option").as_ref(),
            ),
            &fw_form,
        );
        page_box.append(fw_section.widget());

        // Miscellaneous
        let misc_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let simple_container = gtk4::Switch::new();
        misc_form.add_row(
            tr!("Auto open Link Containers (dlc,rsdf,...)").as_ref(),
            None,
            &simple_container,
        );

        let misc_section = ConfigSection::new(
            crate::gui::icon_key::ICON_SETTINGS,
            tr!("Miscellaneous").as_ref(),
            None,
            &misc_form,
        );
        page_box.append(misc_section.widget());

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page_box));
        scrolled.set_vexpand(true);

        // Save each setting as soon as it changes (all calls run in a background
        // thread via api_fire to avoid blocking the GTK main thread).
        let general_save = general.clone();
        download_folder.connect_activate(move |s| {
            let val = s.text().to_string();
            let g = general_save.clone();
            api_fire(move || { let _ = g.set_default_download_folder(&val); });
        });

        let browse_download = download_folder.clone();
        let general_browse = general.clone();
        browse_btn.connect_clicked(move |_| {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title(tr!("Select Folder").as_ref());
            let browse_download = browse_download.clone();
            let general_browse = general_browse.clone();
            glib::MainContext::default().spawn_local(async move {
                match dialog.select_folder_future(None::<&gtk4::Window>).await {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            let path_str = path.to_string_lossy().to_string();
                            browse_download.set_text(&path_str);
                            let g = general_browse.clone();
                            let p = path_str.clone();
                            api_fire(move || { let _ = g.set_default_download_folder(&p); });
                        }
                    }
                    Err(e) => log::warn!("Failed to select folder: {}", e),
                }
            });
        });

        // Chunks/parallel downloads/parallel per host: read from and
        // written through the shared `DownloadLimitsCache` (also shown in
        // the Downloads list's own Quick Settings menu), rather than
        // polling JDownloader independently — see `DownloadLimitsCache`.
        // `refresh_limits` re-reads the cache (free once loaded) each time
        // this page's tab is reopened, so it can't go stale relative to a
        // change made in the Downloads list's Quick Settings menu while
        // this tab stayed closed.
        let loading_limits = Rc::new(Cell::new(true));
        let refresh_limits: Rc<dyn Fn()> = {
            let download_limits = download_limits.clone();
            let loading_limits = loading_limits.clone();
            let max_sim_c = max_sim.clone();
            let per_host_switch_c = per_host_switch.clone();
            let per_host_spin_c = per_host_spin.clone();
            let max_chunks_c = max_chunks.clone();
            Rc::new(move || {
                loading_limits.set(true);
                download_limits.read({
                    let loading_limits = loading_limits.clone();
                    let max_sim_c = max_sim_c.clone();
                    let per_host_switch_c = per_host_switch_c.clone();
                    let per_host_spin_c = per_host_spin_c.clone();
                    let max_chunks_c = max_chunks_c.clone();
                    move |limits: DownloadLimits| {
                        max_sim_c.set_value(limits.max_simultaneous as f64);
                        per_host_switch_c.set_active(limits.max_simultaneous_per_host_enabled);
                        per_host_spin_c.set_value(limits.max_simultaneous_per_host as f64);
                        per_host_spin_c.set_sensitive(limits.max_simultaneous_per_host_enabled);
                        max_chunks_c.set_value(limits.max_chunks as f64);
                        loading_limits.set(false);
                    }
                });
            })
        };
        refresh_limits();
        // `map` fires every time this page's widget actually becomes
        // visible again — unlike hooking `SettingsPanel::toggle()`, which
        // only ran on the closed→open transition and missed the case
        // where the tab was already open and the user just switched back
        // to it via the tab strip directly.
        scrolled.connect_map({
            let refresh_limits = refresh_limits.clone();
            move |_| refresh_limits()
        });

        max_sim.connect_value_changed({
            let download_limits = download_limits.clone();
            let loading_limits = loading_limits.clone();
            move |s| {
                if loading_limits.get() {
                    return;
                }
                let val = s.value() as i32;
                download_limits.update(|limits| limits.max_simultaneous = val);
            }
        });

        per_host_switch.connect_state_notify({
            let download_limits = download_limits.clone();
            let loading_limits = loading_limits.clone();
            let per_host_spin_c = per_host_spin.clone();
            move |s| {
                let active = s.is_active();
                per_host_spin_c.set_sensitive(active);
                if loading_limits.get() {
                    return;
                }
                download_limits.update(|limits| limits.max_simultaneous_per_host_enabled = active);
            }
        });

        per_host_spin.connect_value_changed({
            let download_limits = download_limits.clone();
            let loading_limits = loading_limits.clone();
            move |s| {
                if loading_limits.get() {
                    return;
                }
                let val = s.value() as i32;
                download_limits.update(|limits| limits.max_simultaneous_per_host = val);
            }
        });

        max_chunks.connect_value_changed({
            let download_limits = download_limits.clone();
            let loading_limits = loading_limits.clone();
            move |s| {
                if loading_limits.get() {
                    return;
                }
                let val = s.value() as i32;
                download_limits.update(|limits| limits.max_chunks = val);
            }
        });

        let general_save = general.clone();
        let cleanup_api = cleanup_api;
        remove_combo.connect_selected_notify(move |s| {
            let idx = s.selected() as usize;
            if idx < cleanup_api.len() {
                let val = cleanup_api[idx];
                let g = general_save.clone();
                api_fire(move || { let _ = g.set_cleanup_after_download_action(val); });
            }
        });

        let general_save = general.clone();
        let exists_api = exists_api;
        exists_combo.connect_selected_notify(move |s| {
            let idx = s.selected() as usize;
            if idx < exists_api.len() {
                let val = exists_api[idx];
                let g = general_save.clone();
                api_fire(move || { let _ = g.set_if_file_exists_action(val); });
            }
        });

        let general_save = general.clone();
        let auto_api = auto_api;
        auto_combo.connect_selected_notify(move |s| {
            let idx = s.selected() as usize;
            if idx < auto_api.len() {
                let val = auto_api[idx];
                let g = general_save.clone();
                api_fire(move || { let _ = g.set_auto_start_download_option(val); });
            }
        });

        let countdown_spin_c = countdown_spin.clone();
        let general_save = general.clone();
        countdown_switch.connect_state_notify(move |s| {
            let active = s.is_active();
            countdown_spin_c.set_sensitive(active);
            let g = general_save.clone();
            api_fire(move || { let _ = g.set_show_countdown_on_auto_start_downloads(active); });
        });

        let general_save = general.clone();
        countdown_spin.connect_value_changed(move |s| {
            let val = s.value() as i32;
            let g = general_save.clone();
            api_fire(move || { let _ = g.set_auto_start_countdown_seconds(val); });
        });

        let linkgrabber_save = linkgrabber.clone();
        various_package.connect_state_notify(move |s| {
            let active = s.is_active();
            let lg = linkgrabber_save.clone();
            api_fire(move || { let _ = lg.set_various_package_enabled(active); });
        });

        let general_save = general.clone();
        auto_crc.connect_state_notify(move |s| {
            let active = s.is_active();
            let g = general_save.clone();
            api_fire(move || { let _ = g.set_hash_check_enabled(active); });
        });

        let general_save = general.clone();
        restart_crc.connect_state_notify(move |s| {
            let active = s.is_active();
            let g = general_save.clone();
            api_fire(move || { let _ = g.set_hash_retry_enabled(active); });
        });

        let general_save = general.clone();
        simple_container.connect_state_notify(move |s| {
            let active = s.is_active();
            let g = general_save.clone();
            api_fire(move || { let _ = g.set_auto_open_container_after_download(active); });
        });

        // Loads the remaining settings from the internal JDownloader API
        // once (JDownloader never changes these on its own, and nothing
        // else in gDownloader edits them, so a single one-time load is
        // enough — unlike chunks/parallel downloads/parallel per host
        // above, which the Downloads list's Quick Settings menu can also
        // change and so need the shared, always-current cache instead).
        let (tx, rx) = async_channel::bounded::<GeneralSettingsData>(1);
        let general_for_load = general.clone();
        let linkgrabber_for_load = linkgrabber.clone();
        thread::spawn(move || {
            if let Some(settings) = load_settings_from_api(&general_for_load, &linkgrabber_for_load) {
                let _ = tx.try_send(settings);
            }
        });

        let download_folder_c = download_folder.clone();
        let remove_combo_c = remove_combo.clone();
        let exists_combo_c = exists_combo.clone();
        let auto_combo_c = auto_combo.clone();
        let countdown_switch_c = countdown_switch.clone();
        let countdown_spin_c = countdown_spin.clone();
        let various_package_c = various_package.clone();
        let auto_crc_c = auto_crc.clone();
        let restart_crc_c = restart_crc.clone();
        let simple_container_c = simple_container.clone();

        glib::MainContext::default().spawn_local(async move {
            if let Ok(settings) = rx.recv().await {
                download_folder_c.set_text(&settings.download_folder);
                select_from_api(&remove_combo_c, &cleanup_api, &settings.cleanup_after_download);
                select_from_api(&exists_combo_c, &exists_api, &settings.if_file_exists);
                select_from_api(&auto_combo_c, &auto_api, &settings.auto_start);
                countdown_switch_c.set_active(settings.countdown_enabled);
                countdown_spin_c.set_value(settings.countdown as f64);
                countdown_spin_c.set_sensitive(settings.countdown_enabled);
                various_package_c.set_active(settings.various_package);
                auto_crc_c.set_active(settings.hash_check);
                restart_crc_c.set_active(settings.hash_retry);
                simple_container_c.set_active(settings.open_container);
            }
        });

        Self { widget: scrolled }
    }
}
