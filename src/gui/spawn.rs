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

/// Runs a blocking operation in a background thread (fire-and-forget).
///
/// Use this for API `set_*` calls where the result doesn't need to update the UI.
pub fn api_fire<F>(work: F)
where
    F: FnOnce() + Send + 'static,
{
    std::thread::spawn(work);
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
