use std::path::{Path, PathBuf};
use log::warn;

/// Local data directory where JDownloader is copied and run from.
fn local_jar_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("gdownloader").join("jdownloader"))
}

/// Returns the path to a source JDownloader jar we can bundle/copy.
fn source_jar() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("data/resources/jd/JDownloader.jar"),
        PathBuf::from("data/JDownloader.jar"),
        PathBuf::from("JDownloader.jar"),
    ];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("data/resources/jd/JDownloader.jar"));
            candidates.push(exe_dir.join("data/JDownloader.jar"));
            candidates.push(exe_dir.join("../share/gdownloader/data/JDownloader.jar"));
        }
    }

    for c in candidates {
        if c.exists() {
            return Some(c);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let sys = home.join("Documents/Projects/jdownloader/dev/JDownloader.jar");
        if sys.exists() {
            return Some(sys);
        }
    }

    None
}

/// Returns `true` if `source` has been modified more recently than `target`.
fn is_newer(source: &Path, target: &Path) -> Option<bool> {
    let src_meta = std::fs::metadata(source).ok()?;
    let tgt_meta = std::fs::metadata(target).ok()?;
    let src_time = src_meta.modified().ok()?;
    let tgt_time = tgt_meta.modified().ok()?;
    Some(src_time > tgt_time)
}

/// Finds JDownloader.jar in the following order:
/// 1. The `JDOWNLOADER_JAR` environment variable (development mode, no copy)
/// 2. The file at `data/JDownloader.jar`, copied into the local data directory
/// 3. The system JDownloader working-copy jar, copied into the local data directory
///
/// The goal is to run JDownloader from its own directory in
/// `~/.local/share/gdownloader/jdownloader/`, with its own local configuration.
pub fn find_jar() -> Option<PathBuf> {
    // Development mode: use the indicated jar without copying it.
    if let Ok(p) = std::env::var("JDOWNLOADER_JAR") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    let source = source_jar()?;
    let local_dir = local_jar_dir()?;
    let local_jar = local_dir.join("JDownloader.jar");

    if let Err(e) = std::fs::create_dir_all(&local_dir) {
        warn!(
            "{}",
            tr!(
                "Unable to create local JDownloader directory {}: {}",
                local_dir.display(),
                e
            )
        );
        return None;
    }

    let should_copy = !local_jar.exists() || is_newer(&source, &local_jar).unwrap_or(false);
    if should_copy {
        if let Err(e) = std::fs::copy(&source, &local_jar) {
            warn!(
                "{}",
                tr!(
                    "Unable to copy JDownloader.jar to {}: {}",
                    local_jar.display(),
                    e
                )
            );
            return None;
        }
    }

    Some(local_jar)
}
