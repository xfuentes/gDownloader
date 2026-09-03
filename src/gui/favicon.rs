use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use image::ImageFormat;
use log::{info, warn};
use reqwest::blocking::Client;

use crate::jd::{find_jar, JdApi};

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
        .join("fav");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn cache_path(host: &str) -> Result<PathBuf> {
    Ok(cache_dir()?.join(format!("{host}.png")))
}

pub fn cached(host: &str) -> Option<PathBuf> {
    cache_path(host).ok().filter(|p| p.exists())
}

fn fetch_url(url: &str) -> Result<Vec<u8>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let response = client
        .get(url)
        .send()?
        .error_for_status()?;
    Ok(response.bytes()?.to_vec())
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
}

fn convert_to_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory_with_format(bytes, ImageFormat::Ico)
        .or_else(|_| image::load_from_memory(bytes))?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)?;
    Ok(out)
}

fn fetch_from_site(host: &str) -> Result<Vec<u8>> {
    let mut tried = Vec::new();
    for path in ["favicon.png", "favicon.ico"] {
        let url = format!("https://{}/{}", host, path);
        tried.push(url.clone());
        if let Ok(bytes) = fetch_url(&url) {
            info!("favicon fetched from {} ({} bytes)", url, bytes.len());
            if is_png(&bytes) || path == "favicon.png" {
                return Ok(bytes);
            }
            return convert_to_png(&bytes);
        }
    }
    anyhow::bail!("No favicon found on {}: tried {:?}", host, tried)
}

pub fn resolve(api: &JdApi, host: &str) -> Result<PathBuf> {
    if let Some(p) = cached(host) {
        info!("favicon cache hit: {}", p.display());
        return Ok(p);
    }

    // Ask JDownloader to download the favicon.
    if let Err(e) = api.trigger_favicon(host) {
        warn!("JDownloader favicon trigger failed for {}: {}", host, e);
    } else {
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            thread::sleep(Duration::from_millis(500));
            if let Some(p) = cached(host) {
                info!("favicon provided by JD: {}", p.display());
                return Ok(p);
            }
        }
    }

    // Fallback: fetch directly from the hoster's website.
    info!("favicon not provided by JD, fetching from site for {}", host);
    let bytes = fetch_from_site(host)?;
    let path = cache_path(host)?;
    fs::write(&path, &bytes)?;
    info!("favicon saved: {} ({} bytes)", path.display(), bytes.len());
    Ok(path)
}
