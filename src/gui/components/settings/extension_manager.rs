use adw::prelude::*;
use gtk4::glib;
use log::warn;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::gui::components::{ConfigSection, ConfigSectionForm};
use crate::gui::dialogs::WaitDialog;
use crate::gui::jd_icon;
use crate::gui::spawn::{api_call, api_fire};
use crate::jd::{
    ExtensionQuery, ExtensionStorable, JdApi, JdExtensions, JdProcess, INTERNAL_JD_PORT,
};

fn wait_ready(extensions: &JdExtensions) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if extensions.is_ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Fetches the extension list, retrying for `retry_for` since JDownloader
/// populates `ExtensionController` asynchronously (its own GUI waits on
/// `SecondLevelLaunch.EXTENSIONS_LOADED`) well after the deprecated API
/// starts responding — an immediate single call can race a still-empty list.
fn load_list(extensions: &JdExtensions, retry_for: Duration) -> Vec<ExtensionStorable> {
    if !wait_ready(extensions) {
        return Vec::new();
    }
    let deadline = Instant::now() + retry_for;
    loop {
        match extensions.list(&ExtensionQuery::all()) {
            Ok(list) if !list.is_empty() => return list,
            Ok(_) => {}
            Err(e) => warn!("extensions/list failed: {}", e),
        }
        if Instant::now() >= deadline {
            return Vec::new();
        }
        thread::sleep(Duration::from_millis(800));
    }
}

/// Shared handles for installing/removing extensions and restarting the
/// local JDownloader process, needed since (unlike enabling/disabling) the
/// RemoteAPI has no way to finish those operations without a restart.
#[derive(Clone)]
struct ExtCtx {
    api: Arc<JdApi>,
    extensions: JdExtensions,
    process: Arc<Mutex<JdProcess>>,
    /// `<JD home>/extensions`, the directory JDownloader loads extension
    /// jars from. `None` if the bundled jar could not be located.
    extensions_dir: Option<PathBuf>,
    jar_path: Option<PathBuf>,
}

impl ExtCtx {
    /// Stops and restarts the local JDownloader process, blocking (on a
    /// background thread) until it responds again — mirroring how
    /// JDownloader's own Extension Manager requires a restart after an
    /// install/uninstall before the change actually takes effect.
    fn restart(&self, on_done: impl FnOnce(bool) + 'static) {
        let Some(jar_path) = self.jar_path.clone() else {
            on_done(false);
            return;
        };
        let api = self.api.clone();
        let process = self.process.clone();
        let (tx, rx) = async_channel::bounded::<bool>(1);
        thread::spawn(move || {
            let _ = api.system_exit();
            if let Ok(mut p) = process.lock() {
                let _ = p.stop(true);
            }
            let started = process
                .lock()
                .map(|mut p| p.start(&jar_path).is_ok())
                .unwrap_or(false);
            let mut ready = false;
            if started {
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(60) {
                    if api.is_ready() {
                        ready = true;
                        break;
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
            let _ = tx.try_send(ready);
        });
        glib::MainContext::default().spawn_local(async move {
            let ready = rx.recv().await.unwrap_or(false);
            on_done(ready);
        });
    }
}

/// Locates the on-disk jar for an installed extension so it can be removed.
///
/// JDownloader's RemoteAPI has no `extensions/uninstall` endpoint — the id
/// gDownloader gets back for an installed extension is its Java classname
/// (e.g. `org.jdownloader.extensions.eventscripter.EventScripterExtension`),
/// while the file JDownloader actually loads from `<JD home>/extensions/`
/// is named after the simple class name minus its `Extension` suffix (e.g.
/// `EventScripter.jar`). Extensions built into JDownloader.jar itself have
/// no separate file and simply won't match anything here — intentional,
/// since gDownloader must never guess and delete the wrong file.
fn find_extension_jar(extensions_dir: &std::path::Path, id: &str) -> Option<PathBuf> {
    let simple = id.rsplit('.').next().unwrap_or(id);
    let stripped = simple.strip_suffix("Extension").unwrap_or(simple);
    let entries = std::fs::read_dir(extensions_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jar") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if stem.eq_ignore_ascii_case(simple) || stem.eq_ignore_ascii_case(stripped) {
                return Some(path);
            }
        }
    }
    None
}

pub struct ExtensionManagerPage;

impl ExtensionManagerPage {
    pub fn build(
        process: Arc<Mutex<JdProcess>>,
        jar_path: Option<PathBuf>,
    ) -> gtk4::ScrolledWindow {
        let api = Arc::new(JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT)));
        let extensions = JdExtensions::new(Arc::clone(&api));
        let extensions_dir = jar_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.join("extensions"));
        let ctx = ExtCtx {
            api,
            extensions: extensions.clone(),
            process,
            extensions_dir,
            jar_path,
        };

        let page_box = gtk4::Box::new(gtk4::Orientation::Vertical, 15);
        page_box.set_margin_top(15);
        page_box.set_margin_bottom(15);
        page_box.set_margin_start(15);
        page_box.set_margin_end(15);
        page_box.set_hexpand(true);
        page_box.set_vexpand(true);

        let label_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let checkbox_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let input_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        let empty_form =
            ConfigSectionForm::new(&label_size_group, &checkbox_size_group, &input_size_group);

        let header_section = ConfigSection::new(
            crate::gui::icon_key::ICON_EXTENSIONMANAGER,
            tr!("Extension Manager").as_ref(),
            Some(tr!("Enable, disable or install JDownloader extensions.").as_ref()),
            &empty_form,
        );

        let refresh_btn = gtk4::Button::from_icon_name(crate::gui::icon_key::ICON_REFRESH);
        refresh_btn.set_tooltip_text(Some(tr!("Refresh").as_ref()));
        refresh_btn.set_valign(gtk4::Align::Start);

        let header_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        header_row.append(header_section.widget());
        header_row.append(&refresh_btn);
        page_box.append(&header_row);

        // Shared so every row's Install/Remove button gets the same width,
        // regardless of which of the two (differently-sized) labels it has.
        let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        let list_box = gtk4::ListBox::new();
        list_box.set_selection_mode(gtk4::SelectionMode::None);
        list_box.add_css_class("boxed-list");

        let placeholder = gtk4::Label::new(Some(tr!("Loading extensions…").as_ref()));
        placeholder.add_css_class("dim-label");
        placeholder.set_margin_top(24);
        placeholder.set_margin_bottom(24);
        list_box.set_placeholder(Some(&placeholder));

        let scroll = gtk4::ScrolledWindow::builder()
            .child(&list_box)
            .vexpand(true)
            .hexpand(true)
            .min_content_height(320)
            .propagate_natural_height(true)
            .build();
        page_box.append(&scroll);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page_box));
        scrolled.set_vexpand(true);

        let (tx, rx) = async_channel::bounded::<Vec<ExtensionStorable>>(1);

        let load = {
            let extensions = extensions.clone();
            let tx = tx.clone();
            move |retry_for: Duration| {
                let extensions = extensions.clone();
                let tx = tx.clone();
                thread::spawn(move || {
                    let list = load_list(&extensions, retry_for);
                    let _ = tx.try_send(list);
                });
            }
        };
        load(Duration::from_secs(25));

        let load_c = load.clone();
        refresh_btn.connect_clicked(move |_| load_c(Duration::from_secs(3)));

        glib::MainContext::default().spawn_local(async move {
            while let Ok(list) = rx.recv().await {
                if list.is_empty() {
                    placeholder.set_label(tr!("No extensions found.").as_ref());
                }
                list_box.remove_all();
                for ext in &list {
                    list_box.append(&build_row(
                        &ctx,
                        ext,
                        {
                            let load = load.clone();
                            move || load(Duration::from_secs(3))
                        },
                        &button_size_group,
                    ));
                }
            }
        });

        scrolled
    }
}

fn build_row(
    ctx: &ExtCtx,
    ext: &ExtensionStorable,
    reload: impl Fn() + Clone + 'static,
    button_size_group: &gtk4::SizeGroup,
) -> gtk4::ListBoxRow {
    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    row_box.set_margin_top(8);
    row_box.set_margin_bottom(8);
    row_box.set_margin_start(12);
    row_box.set_margin_end(12);

    let icon_gicon = ext
        .icon_key
        .as_deref()
        .map(jd_icon::resolve)
        .unwrap_or_else(|| jd_icon::resolve(crate::gui::icon_key::ICON_EXTENSION));
    let icon = gtk4::Image::from_gicon(&icon_gicon);
    icon.set_pixel_size(32);
    icon.set_valign(gtk4::Align::Center);
    row_box.append(&icon);

    let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    text_box.set_hexpand(true);
    text_box.set_valign(gtk4::Align::Center);

    let name_label = gtk4::Label::new(Some(ext.name.as_deref().unwrap_or(&ext.id)));
    name_label.set_halign(gtk4::Align::Start);
    name_label.set_xalign(0.0);
    name_label.add_css_class("heading");
    text_box.append(&name_label);

    if let Some(desc) = ext.description.as_deref().filter(|d| !d.is_empty()) {
        let desc_label = gtk4::Label::new(Some(desc));
        desc_label.set_halign(gtk4::Align::Start);
        desc_label.set_xalign(0.0);
        desc_label.set_wrap(true);
        desc_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
        desc_label.add_css_class("dim-label");
        text_box.append(&desc_label);
    }
    row_box.append(&text_box);

    // Enabled/disabled checkbox: unchecked and locked while the extension
    // isn't installed, matching JDownloader's own Extension Manager.
    let checkbox = gtk4::CheckButton::new();
    checkbox.set_valign(gtk4::Align::Center);
    checkbox.set_active(ext.installed && ext.enabled);
    checkbox.set_sensitive(ext.installed);
    if ext.installed {
        let extensions_c = ctx.extensions.clone();
        let id = ext.id.clone();
        checkbox.connect_toggled(move |c| {
            let active = c.is_active();
            let e = extensions_c.clone();
            let id = id.clone();
            api_fire(move || {
                let _ = e.set_enabled(&id, active);
            });
        });
    }
    row_box.append(&checkbox);

    if ext.installed {
        let jar_file = ctx
            .extensions_dir
            .as_deref()
            .and_then(|dir| find_extension_jar(dir, &ext.id));

        let remove_btn = gtk4::Button::with_label(tr!("Remove").as_ref());
        remove_btn.set_valign(gtk4::Align::Center);
        button_size_group.add_widget(&remove_btn);

        match jar_file {
            Some(jar_file) => {
                let ctx_c = ctx.clone();
                let reload_c = reload.clone();
                remove_btn.connect_clicked(move |btn| {
                    let Some(root) = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok())
                    else {
                        return;
                    };
                    let wait = WaitDialog::show(&root, tr!("Removing extension…").as_ref());
                    let jar_file = jar_file.clone();
                    let ctx_c2 = ctx_c.clone();
                    let reload_c2 = reload_c.clone();
                    api_call(move || std::fs::remove_file(&jar_file), move |result| {
                        if let Err(e) = result {
                            warn!("failed to remove extension jar: {}", e);
                            wait.close();
                            reload_c2();
                            return;
                        }
                        ctx_c2.restart(move |ready| {
                            if !ready {
                                warn!("JDownloader did not come back up after extension removal");
                            }
                            wait.close();
                            reload_c2();
                        });
                    });
                });
            }
            None => {
                remove_btn.set_sensitive(false);
                remove_btn.set_tooltip_text(Some(
                    tr!("This extension is built into JDownloader and cannot be removed.")
                        .as_ref(),
                ));
            }
        }
        row_box.append(&remove_btn);
    } else {
        let install_btn = gtk4::Button::with_label(tr!("Install").as_ref());
        install_btn.set_valign(gtk4::Align::Center);
        button_size_group.add_widget(&install_btn);

        let ctx_c = ctx.clone();
        let id = ext.id.clone();
        let reload_c = reload.clone();
        install_btn.connect_clicked(move |btn| {
            let Some(root) = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) else {
                return;
            };
            let wait = WaitDialog::show(&root, tr!("Installing extension…").as_ref());
            let e = ctx_c.extensions.clone();
            let id = id.clone();
            let ctx_c2 = ctx_c.clone();
            let reload_c2 = reload_c.clone();
            api_call(move || e.install(&id), move |result| {
                if !matches!(result, Ok(true)) {
                    warn!("extensions/install failed: {:?}", result);
                    wait.close();
                    reload_c2();
                    return;
                }
                ctx_c2.restart(move |ready| {
                    if !ready {
                        warn!("JDownloader did not come back up after extension installation");
                    }
                    wait.close();
                    reload_c2();
                });
            });
        });
        row_box.append(&install_btn);
    }

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&row_box));
    row.set_activatable(false);
    row.set_selectable(false);
    row
}
