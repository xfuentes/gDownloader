use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use log::warn;

/// Official JDownloader installer location of the standalone jar.
const JAR_DOWNLOAD_URL: &str = "https://installer.jdownloader.org/JDownloader.jar";

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

    let local_dir = local_jar_dir()?;
    let local_jar = local_dir.join("JDownloader.jar");

    // Without a bundled source, a previously downloaded local jar is used as is.
    let Some(source) = source_jar() else {
        return local_jar.exists().then_some(local_jar);
    };

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

/// Downloads JDownloader.jar into the local data directory, reporting progress as a 0..1 fraction (negative if unknown).
pub fn download_jar(mut on_progress: impl FnMut(f64)) -> anyhow::Result<PathBuf> {
    let local_dir = local_jar_dir().ok_or_else(|| anyhow::anyhow!("No local data directory"))?;
    std::fs::create_dir_all(&local_dir)?;
    let local_jar = local_dir.join("JDownloader.jar");
    let partial = local_dir.join("JDownloader.jar.part");

    let client = reqwest::blocking::Client::builder().timeout(None).build()?;
    let mut response = client.get(JAR_DOWNLOAD_URL).send()?.error_for_status()?;
    let total = response.content_length().filter(|n| *n > 0);

    let mut file = std::fs::File::create(&partial)?;
    let mut buf = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        on_progress(total.map_or(-1.0, |t| (done as f64 / t as f64).min(1.0)));
    }
    file.flush()?;
    drop(file);

    if total.is_some_and(|t| t != done) {
        let _ = std::fs::remove_file(&partial);
        anyhow::bail!("Incomplete download of JDownloader.jar");
    }
    std::fs::rename(&partial, &local_jar)?;
    Ok(local_jar)
}
