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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Count of `api_fire` background writes still in flight, so
/// [`wait_for_pending`] can let them finish before JDownloader is asked to
/// shut down (see its own doc comment for why that race matters).
fn pending() -> &'static AtomicUsize {
    static PENDING: OnceLock<AtomicUsize> = OnceLock::new();
    PENDING.get_or_init(|| AtomicUsize::new(0))
}

/// Runs a blocking operation in a background thread (fire-and-forget).
///
/// Use this for API `set_*` calls where the result doesn't need to update the UI.
pub fn api_fire<F>(work: F)
where
    F: FnOnce() + Send + 'static,
{
    pending().fetch_add(1, Ordering::SeqCst);
    std::thread::spawn(move || {
        work();
        pending().fetch_sub(1, Ordering::SeqCst);
    });
}

/// Blocks the calling thread until every in-flight [`api_fire`] write has
/// completed, or `timeout` elapses.
///
/// JDownloader delays config writes to disk when running headless (how
/// gDownloader always launches it) and only flushes them on its own
/// graceful shutdown — see [`crate::jd::JdProcess::stop`]. But a setting
/// changed right before the window is closed is written via a detached
/// `api_fire` background thread that isn't guaranteed to have even sent
/// its HTTP request yet by the time the close handler asks JDownloader to
/// exit, let alone finished — silently dropping that last change. Call
/// this right before `JdApi::system_exit` to close that race.
pub fn wait_for_pending(timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while pending().load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Runs a blocking operation in a background thread, then calls `on_done` on
/// the GTK main thread with the result.
///
/// Use this when the API result must update the UI (e.g. triggering a refresh).
pub fn api_call<F, T, C>(work: F, on_done: C)
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
    C: FnOnce(T) + 'static,
{
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.try_send(work());
    });
    gtk4::glib::MainContext::default().spawn_local(async move {
        if let Ok(result) = rx.recv().await {
            on_done(result);
        }
    });
}
