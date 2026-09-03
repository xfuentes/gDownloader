use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct LogWatcher {
    log_root: PathBuf,
}

pub struct LogStatus {
    pub text: String,
    pub progress: f64,
}

impl LogWatcher {
    pub fn new(log_root: PathBuf) -> Self {
        Self { log_root }
    }

    pub fn poll(&self) -> Option<LogStatus> {
        let dir = latest_session_dir(&self.log_root)?;
        let mut combined = String::new();
        for name in &[
            "Log.L.log.0",
            "org.jdownloader.update.UpdateManager.log.0",
            "org.jdownloader.update.launcher.SecondLevelLauncher.log.0",
        ] {
            let path = dir.join(name);
            if let Ok(mut f) = fs::File::open(path) {
                let mut s = String::new();
                let _ = f.read_to_string(&mut s);
                combined.push_str(&s);
            }
        }
        Some(parse(&combined))
    }
}

fn latest_session_dir(log_root: &Path) -> Option<PathBuf> {
    let mut best: Option<(PathBuf, SystemTime)> = None;
    for entry in fs::read_dir(log_root).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let log_file = path.join("Log.L.log.0");
        if !log_file.exists() {
            continue;
        }
        let modified = fs::metadata(&log_file).ok()?.modified().ok()?;
        if best.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
            best = Some((path, modified));
        }
    }
    best.map(|(p, _)| p)
}

fn parse(content: &str) -> LogStatus {
    if content.contains("IP [") {
        return LogStatus {
            text: tr!("JDownloader starting, please wait...").to_string(),
            progress: -1.0,
        };
    }
    if content.contains("Self Update successful") {
        return LogStatus {
            text: tr!("Restarting after update").to_string(),
            progress: -1.0,
        };
    }
    if let Some(_) = latest_update_progress(content) {
        let text = latest_update_message(content).unwrap_or_else(|| tr!("Update").to_string());
        return LogStatus { text, progress: -1.0 };
    }
    if content.contains("Application Root:") {
        return LogStatus {
            text: tr!("JDownloader starting").to_string(),
            progress: -1.0,
        };
    }
    LogStatus {
        text: tr!("Initialization").to_string(),
        progress: -1.0,
    }
}

fn latest_update_progress(content: &str) -> Option<f64> {
    let key = "Update Progress: ";
    let mut last = None;
    let mut start = 0;
    while let Some(idx) = content[start..].find(key) {
        let pos = start + idx + key.len();
        let rest = &content[pos..];
        if let Some(end) = rest.find('%') {
            if let Ok(n) = rest[..end].trim().parse::<i32>() {
                last = Some(if n >= 0 { n as f64 / 100.0 } else { 0.05 });
            }
        }
        start = pos;
    }
    last
}

fn latest_update_message(content: &str) -> Option<String> {
    let key = "Update Message: ";
    let mut start = 0;
    let mut message = None;
    while let Some(idx) = content[start..].find(key) {
        let pos = start + idx + key.len();
        let rest = &content[pos..];
        let end = rest.find('\n').unwrap_or(rest.len());
        message = Some(rest[..end].trim().to_string());
        start = pos + end + 1;
    }
    message
}
