use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;

use crate::jd::{GeneralSettings, JdApi};

/// The subset of `GeneralSettings` that's shown in *two* places at once
/// (the Downloads list's own Quick Settings menu, and Settings > General) —
/// the only fields that actually need a shared, cached source of truth.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DownloadLimits {
    pub max_chunks: i32,
    pub max_simultaneous: i32,
    pub max_simultaneous_per_host_enabled: bool,
    pub max_simultaneous_per_host: i32,
}

struct Inner {
    state: Option<DownloadLimits>,
    loading: bool,
    waiters: Vec<Box<dyn FnOnce(DownloadLimits)>>,
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
                waiters: Vec::new(),
            })),
        }
    }

    /// The one read accessor: hands the current values to `on_ready`.
    /// Synchronous (no network) once the cache has been loaded once —
    /// including every call after the very first, so re-reading on, say,
    /// reopening a settings tab never re-fetches from JDownloader.
    pub fn read(&self, on_ready: impl FnOnce(DownloadLimits) + 'static) {
        let mut inner = self.inner.borrow_mut();
        if let Some(state) = inner.state {
            drop(inner);
            on_ready(state);
            return;
        }
        inner.waiters.push(Box::new(on_ready));
        if inner.loading {
            return;
        }
        inner.loading = true;
        drop(inner);

        let api = self.api.clone();
        let inner_handle = self.inner.clone();
        let (tx, rx) = async_channel::bounded::<DownloadLimits>(1);
        std::thread::spawn(move || {
            // The very first `read()` typically fires while building the
            // UI, well before JDownloader's own RemoteAPI is actually up —
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
            };
            let _ = tx.try_send(state);
        });
        glib::MainContext::default().spawn_local(async move {
            if let Ok(state) = rx.recv().await {
                let waiters = {
                    let mut inner = inner_handle.borrow_mut();
                    inner.state = Some(state);
                    inner.loading = false;
                    std::mem::take(&mut inner.waiters)
                };
                for waiter in waiters {
                    waiter(state);
                }
            }
        });
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
            crate::gui::spawn::api_fire(move || {
                let _ = api.set_max_simultane_downloads_per_host(v);
            });
        }
    }
}
