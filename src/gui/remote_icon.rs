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

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use gtk4::gio;
use gtk4::glib::prelude::*;

use crate::jd::{find_jar, JdApi};

/// Caches icons resolved from opaque JDownloader icon keys (e.g.
/// `"kc.<hash>"` — server-side composited icons with no bundled PNG
/// equivalent), fetched via `/contentV2/getIcon`. JDownloader hands these
/// out anywhere it can't describe an icon with a plain theme filename: a
/// link's variant icon (`variant/iconKey`), a package's or link's merged
/// status icon (`statusIconKey`, e.g. a progress icon while downloading),
/// and potentially others.
fn cache_dir() -> Result<PathBuf> {
    let jar = find_jar().context("JDownloader.jar not found")?;
    let dir = jar
        .parent()
        .context("JDownloader.jar has no parent directory")?
        .join("themes")
        .join("standard")
        .join("org")
        .join("jdownloader")
        .join("images")
        .join("remote");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn sanitize(key: &str) -> String {
    key.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect()
}

fn cache_path(key: &str) -> Result<PathBuf> {
    Ok(cache_dir()?.join(format!("{}.png", sanitize(key))))
}

pub fn cached(key: &str) -> Option<PathBuf> {
    cache_path(key).ok().filter(|p| p.exists())
}

/// Resolves an icon key, either from gDownloader's bundled/JD-theme icons
/// (when it's a plain filename) or from this module's fetch cache (when
/// it's an opaque key that had to be fetched via [`fetch`]).
pub fn resolve(key: &str) -> Option<gio::Icon> {
    if key.is_empty() {
        return None;
    }
    let path = crate::gui::jd_icon::resolve_path(key).or_else(|| cached(key))?;
    Some(gio::FileIcon::new(&gio::File::for_path(&path)).upcast::<gio::Icon>())
}

/// Downloads and caches an icon by its opaque key via `/contentV2/getIcon`.
pub fn fetch(api: &JdApi, key: &str) -> Result<PathBuf> {
    let bytes = api.fetch_icon(key, 32)?;
    let path = cache_path(key)?;
    fs::write(&path, &bytes)?;
    Ok(path)
}
