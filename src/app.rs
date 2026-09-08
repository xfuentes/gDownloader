use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::jd::{JdApi, JdProcess, LogWatcher, INTERNAL_JD_PORT};

#[derive(Debug, Clone)]
pub enum JdMessage {
    Started,
    Error(String),
    Status(String, f64),
}

pub fn start_jd(path: PathBuf, process: Arc<Mutex<JdProcess>>, tx: async_channel::Sender<JdMessage>) {
    thread::spawn(move || {
        if let Err(e) = process.lock().unwrap().start(&path) {
            let _ = tx.try_send(JdMessage::Error(tr!("Error: {}", e).to_string()));
            return;
        }

        let api = JdApi::new(format!("http://localhost:{}", INTERNAL_JD_PORT));
        let log_root = path.parent().map(|p| p.join("logs")).unwrap_or_default();
        let watcher = LogWatcher::new(log_root);
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(60) {
            if let Some(st) = watcher.poll() {
                let _ = tx.try_send(JdMessage::Status(st.text, st.progress));
            }
            if api.is_ready() {
                let _ = tx.try_send(JdMessage::Started);
                return;
            }
            thread::sleep(Duration::from_millis(250));
        }

        let _ = process.lock().unwrap().stop(false);
        let _ = tx.try_send(JdMessage::Error(
            tr!("JDownloader did not respond in time").to_string(),
        ));
    });
}
