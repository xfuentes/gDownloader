use std::path::PathBuf;

use gtk4::gio;
use gtk4::glib::prelude::*;
use gtk4::IconTheme;

pub fn resolve_path(name: &str) -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("data/resources/jd/images")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("data/resources/jd/images"));
            candidates.push(exe_dir.join("../share/gdownloader/data/resources/jd/images"));
        }
    }
    if let Some(jar) = crate::jd::find_jar() {
        if let Some(jar_dir) = jar.parent() {
            candidates.push(jar_dir.join("themes").join("standard").join("org").join("jdownloader").join("images"));
            candidates.push(jar_dir.join("themes").join("standard").join("org").join("jdownloader").join("images").join("fav"));
        }
    }
    for base in candidates {
        for sub in ["", "logo", "hoster"] {
            let p = if sub.is_empty() {
                base.join(format!("{name}.png"))
            } else {
                base.join(sub).join(format!("{name}.png"))
            };
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

pub fn resolve(name: &str) -> gio::Icon {
    if let Some(path) = resolve_path(name) {
        let file = gio::File::for_path(&path);
        gio::FileIcon::new(&file).upcast::<gio::Icon>()
    } else {
        gio::ThemedIcon::new(name).upcast::<gio::Icon>()
    }
}

pub fn resolve_or(name: &str, fallback: &str) -> gio::Icon {
    if resolve_path(name).is_some() {
        resolve(name)
    } else if let Some(display) = gtk4::gdk::Display::default() {
        if IconTheme::for_display(&display).has_icon(name) {
            resolve(name)
        } else {
            resolve(fallback)
        }
    } else {
        resolve(fallback)
    }
}
