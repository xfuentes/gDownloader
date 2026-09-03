use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Internal API port used by the JDownloader instance launched by gDownloader.
pub const INTERNAL_JD_PORT: u16 = 5128;

/// JVM marker used to identify internal JDownloader processes.
const JD_INTERNAL_FLAG: &str = "-Dgdownloader.internal=1";
const JD_INTERNAL_PKILL: &str = r"^java.*gdownloader.internal=1";

/// Controls the headless JDownloader process.
pub struct JdProcess {
    child: Option<Child>,
}

impl JdProcess {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn start(&mut self, jar: &Path) -> anyhow::Result<()> {
        if self.child.is_some() {
            anyhow::bail!("{}", tr!("JDownloader is already running"));
        }

        let cwd = jar
            .parent()
            .ok_or_else(|| anyhow::anyhow!("{}", tr!("JDownloader.jar has no parent directory")))?;
        let jar_name = jar
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("{}", tr!("JDownloader.jar has no file name")))?;

        let cfg_dir = cwd.join("cfg");
        std::fs::create_dir_all(&cfg_dir)?;
        let cfg_path = cfg_dir.join("org.jdownloader.api.RemoteAPIConfig.json");
        let mut cfg: serde_json::Value = if cfg_path.exists() {
            match std::fs::read_to_string(&cfg_path) {
                Ok(content) if !content.trim().is_empty() => {
                    serde_json::from_str(&content)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
                }
                _ => serde_json::Value::Object(serde_json::Map::new()),
            }
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert("deprecatedapienabled".to_string(), serde_json::Value::Bool(true));
            obj.insert(
                "deprecatedapiport".to_string(),
                (INTERNAL_JD_PORT as i64).into(),
            );
            obj.insert(
                "deprecatedapilocalhostonly".to_string(),
                serde_json::Value::Bool(true),
            );
            obj.insert(
                "headlessmyjdownloadermandatory".to_string(),
                serde_json::Value::Bool(false),
            );
        }
        std::fs::write(&cfg_path, serde_json::to_string(&cfg)?)?;

        let vmoptions_path = cwd.join("JDownloader2.vmoptions");
        let vmoptions = format!("{}\n-Djava.awt.headless=true\n", JD_INTERNAL_FLAG);
        std::fs::write(&vmoptions_path, vmoptions)?;

        let child = Command::new("java")
            .current_dir(cwd)
            .env("LANG", "C")
            .arg(JD_INTERNAL_FLAG)
            .arg("-Djava.awt.headless=true")
            .arg("-jar")
            .arg(jar_name)
            .arg("-n")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(mut child) = self.child.take() {
            // Gracefully ask JDownloader to shut down so JsonConfig has time to
            // flush pending changes to disk.
            let _ = Command::new("pkill")
                .arg("-TERM")
                .arg("-f")
                .arg(JD_INTERNAL_PKILL)
                .output();
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                if child.try_wait()?.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            if child.try_wait()?.is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        let _ = Command::new("pkill")
            .arg("-9")
            .arg("-f")
            .arg(JD_INTERNAL_PKILL)
            .output();
        Ok(())
    }
}
