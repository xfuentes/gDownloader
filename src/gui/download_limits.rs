use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;

use crate::jd::{GeneralSettings, JdApi};

/// The subset of `GeneralSettings` that's shown in more than one place at
/// once (the Downloads list's own Quick Settings menu, Settings > General,
/// and the main menu bar's Settings menu) — the only fields that actually
/// need a shared, cached source of truth.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DownloadLimits {
    pub max_chunks: i32,
    pub max_simultaneous: i32,
    pub max_simultaneous_per_host_enabled: bool,
    pub max_simultaneous_per_host: i32,
    pub speed_limit_enabled: bool,
    /// Bytes/s (JDownloader's own unit) — divide by 1024 for a KiB/s UI.
    pub speed_limit: i32,
}

struct Inner {
    state: Option<DownloadLimits>,
    loading: bool,
    observers: Vec<Rc<dyn Fn(DownloadLimits)>>,
}

/// Single shared cache for [`DownloadLimits`], so every UI surface that
/// shows these settings reads and writes the same state instead of each
/// independently polling JDownloader (which never changes them on its
/// own — only gDownloader's own UI does, through [`Self::update`]).
#[derive(Clone)]
pub struct DownloadLimitsCache {
    api: GeneralSettings,
    inner: Rc<RefCell<Inner>>,
}

impl DownloadLimitsCache {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self {
            api: GeneralSettings::new(api),
            inner: Rc::new(RefCell::new(Inner {
                state: None,
                loading: false,
                observers: Vec::new(),
            })),
        }
    }

    /// Registers `observer` to be called with the current values right
    /// away (or as soon as they're first loaded), and again every time
    /// [`Self::update`] changes them from *any* surface — this is how a
    /// surface that's currently visible (e.g. Settings > General left
    /// open) stays live-updated without needing to be re-shown first.
    /// Never unregistered — callers are expected to subscribe once from
    /// widgets that live for the whole app session, not from something
    /// rebuilt on every open.
    pub fn subscribe(&self, observer: impl Fn(DownloadLimits) + 'static) {
        let observer: Rc<dyn Fn(DownloadLimits)> = Rc::new(observer);
        let mut inner = self.inner.borrow_mut();
        inner.observers.push(observer.clone());
        if let Some(state) = inner.state {
            drop(inner);
            observer(state);
            return;
        }
        let already_loading = inner.loading;
        inner.loading = true;
        drop(inner);
        if !already_loading {
            self.load();
        }
    }

    fn load(&self) {
        let api = self.api.clone();
        let inner_handle = self.inner.clone();
        let self_for_notify = self.clone();
        let (tx, rx) = async_channel::bounded::<DownloadLimits>(1);
        std::thread::spawn(move || {
            // The very first load typically fires while building the UI,
            // well before JDownloader's own RemoteAPI is actually up —
            // without this wait, every getter below fails and silently
            // falls back to its hardcoded default (e.g. `unwrap_or(3)`),
            // caching that wrong value instead of the real one.
            let start = std::time::Instant::now();
            while !api.is_ready() && start.elapsed() < std::time::Duration::from_secs(30) {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            let state = DownloadLimits {
                max_chunks: api.get_max_chunks_per_file().unwrap_or(1),
                max_simultaneous: api.get_max_simultane_downloads().unwrap_or(3),
                max_simultaneous_per_host_enabled: api
                    .get_max_downloads_per_host_enabled()
                    .unwrap_or(false),
                max_simultaneous_per_host: api.get_max_simultane_downloads_per_host().unwrap_or(1),
                speed_limit_enabled: api.get_download_speed_limit_enabled().unwrap_or(false),
                speed_limit: api.get_download_speed_limit().unwrap_or(50 * 1024),
            };
            let _ = tx.try_send(state);
        });
        glib::MainContext::default().spawn_local(async move {
            if let Ok(state) = rx.recv().await {
                {
                    let mut inner = inner_handle.borrow_mut();
                    inner.state = Some(state);
                    inner.loading = false;
                }
                self_for_notify.notify_observers(state);
            }
        });
    }

    fn notify_observers(&self, state: DownloadLimits) {
        let observers = self.inner.borrow().observers.clone();
        for observer in observers {
            observer(state);
        }
    }

    /// The one write accessor: applies `f` to the cached state, then
    /// persists to JDownloader exactly the fields `f` actually changed.
    pub fn update(&self, f: impl FnOnce(&mut DownloadLimits)) {
        let mut inner = self.inner.borrow_mut();
        let old = inner.state.unwrap_or_default();
        let mut new = old;
        f(&mut new);
        inner.state = Some(new);
        drop(inner);

        if old != new {
            self.notify_observers(new);
        }

        // Routed through `api_fire` (rather than a bare `thread::spawn`) so
        // the app's shutdown handler can wait for this write to actually
        // land before asking JDownloader to exit — see
        // `crate::gui::spawn::wait_for_pending`.
        let api = self.api.clone();
        if old.max_chunks != new.max_chunks {
            let v = new.max_chunks;
            let api = api.clone();
            crate::gui::spawn::api_fire(move || {
                let _ = api.set_max_chunks_per_file(v);
            });
        }
        if old.max_simultaneous != new.max_simultaneous {
            let v = new.max_simultaneous;
            let api = api.clone();
            crate::gui::spawn::api_fire(move || {
                let _ = api.set_max_simultane_downloads(v);
            });
        }
        if old.max_simultaneous_per_host_enabled != new.max_simultaneous_per_host_enabled {
            let v = new.max_simultaneous_per_host_enabled;
            let api = api.clone();
            crate::gui::spawn::api_fire(move || {
                let _ = api.set_max_downloads_per_host_enabled(v);
            });
        }
        if old.max_simultaneous_per_host != new.max_simultaneous_per_host {
            let v = new.max_simultaneous_per_host;
            let api = api.clone();
            crate::gui::spawn::api_fire(move || {
                let _ = api.set_max_simultane_downloads_per_host(v);
            });
        }
        if old.speed_limit_enabled != new.speed_limit_enabled {
            let v = new.speed_limit_enabled;
            let api = api.clone();
            crate::gui::spawn::api_fire(move || {
                let _ = api.set_download_speed_limit_enabled(v);
            });
        }
        if old.speed_limit != new.speed_limit {
            let v = new.speed_limit;
            crate::gui::spawn::api_fire(move || {
                let _ = api.set_download_speed_limit(v);
            });
        }
    }
}
