//! Obscura (https://github.com/h4ckf0r0day/obscura, Apache-2.0): a small headless browser the `read_url` tool
//! drives as a subprocess. It is not bundled with the installer (~40 MB): the user downloads it from
//! Configuración > Búsqueda web, the same way the Whisper models are downloaded.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use aivt_eventbus::{AppEvent, EventBus};

use crate::error::ToolError;

const VERSION: &str = "v0.2.5";
/// "no-render" is enough to turn a page into text and is ~28 MB lighter than the full build; no stealth build.
const ASSET: &str = "obscura-x86_64-windows-no-render.zip";
/// SHA-256 of the release asset (GitHub's own `digest` for v0.2.5), checked before anything is unpacked.
const ASSET_SHA256: &str = "cb3ba733017637341256adc6eb52f9caaf85423535092bdd31b4a90abc710348";
/// The zip holds exactly these two files; nothing else is ever extracted.
const FILES: [&str; 2] = ["obscura.exe", "obscura-worker.exe"];
pub const APPROX_SIZE_MB: u32 = 41;
const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

struct Inner {
    dir: PathBuf,
    client: reqwest::Client,
    event_bus: EventBus,
    downloading: Mutex<()>,
}

#[derive(Clone)]
pub struct ObscuraManager(Arc<Inner>);

impl ObscuraManager {
    pub fn new(dir: PathBuf, event_bus: EventBus) -> Self {
        Self(Arc::new(Inner { dir, client: reqwest::Client::new(), event_bus, downloading: Mutex::new(()) }))
    }

    /// Only the Windows x86_64 build is wired up so far.
    pub fn is_supported() -> bool {
        cfg!(all(windows, target_arch = "x86_64"))
    }

    pub fn exe_path(&self) -> PathBuf {
        self.0.dir.join(FILES[0])
    }

    pub fn is_installed(&self) -> bool {
        FILES.iter().all(|name| self.0.dir.join(name).is_file())
    }

    pub async fn delete(&self) -> Result<(), ToolError> {
        for name in FILES {
            let path = self.0.dir.join(name);
            if path.is_file() {
                tokio::fs::remove_file(&path).await.map_err(|err| ToolError::Install(err.to_string()))?;
            }
        }
        Ok(())
    }

    /// Downloads the zip to a `.part` file, checks its SHA-256, unpacks the two executables and publishes
    /// `ObscuraDownload*` events. A failed or half-finished download never leaves a usable-looking install.
    pub async fn download(&self) -> Result<(), ToolError> {
        let _guard = self.0.downloading.lock().await;
        if self.is_installed() {
            return Ok(());
        }
        if !Self::is_supported() {
            let err = ToolError::Install("Obscura is only available for Windows x86_64 for now".to_string());
            self.0.event_bus.publish(AppEvent::ObscuraDownloadError { message: err.to_string() });
            return Err(err);
        }
        let result = self.download_and_unpack().await;
        match &result {
            Ok(()) => self.0.event_bus.publish(AppEvent::ObscuraDownloadFinished),
            Err(err) => {
                let _ = self.delete().await;
                self.0.event_bus.publish(AppEvent::ObscuraDownloadError { message: err.to_string() });
            }
        }
        result
    }

    async fn download_and_unpack(&self) -> Result<(), ToolError> {
        let io = |err: std::io::Error| ToolError::Install(err.to_string());
        tokio::fs::create_dir_all(&self.0.dir).await.map_err(io)?;
        let part = self.0.dir.join("obscura.zip.part");
        let hash = self.download_to(&part).await;
        let outcome = match hash {
            Ok(hash) if hash == ASSET_SHA256 => {
                let (zip_path, dir) = (part.clone(), self.0.dir.clone());
                tokio::task::spawn_blocking(move || unpack(&zip_path, &dir)).await.map_err(|err| ToolError::Install(err.to_string()))?
            }
            Ok(_) => Err(ToolError::Install("the downloaded file does not match the expected checksum".to_string())),
            Err(err) => Err(err),
        };
        let _ = tokio::fs::remove_file(&part).await;
        outcome
    }

    /// Streams the release asset into `part` and returns its SHA-256 (lowercase hex).
    async fn download_to(&self, part: &Path) -> Result<String, ToolError> {
        let url = format!("https://github.com/h4ckf0r0day/obscura/releases/download/{VERSION}/{ASSET}");
        let response = self.0.client.get(url).send().await.map_err(|err| ToolError::Network(err.to_string()))?;
        if !response.status().is_success() {
            return Err(ToolError::Install(format!("HTTP {}", response.status())));
        }
        let total_bytes = response.content_length();
        let mut file = tokio::fs::File::create(part).await.map_err(|err| ToolError::Install(err.to_string()))?;
        let mut hasher = Sha256::new();
        let mut downloaded_bytes = 0u64;
        let mut last_emit = Instant::now();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| ToolError::Network(err.to_string()))?;
            hasher.update(&chunk);
            file.write_all(&chunk).await.map_err(|err| ToolError::Install(err.to_string()))?;
            downloaded_bytes += chunk.len() as u64;
            if last_emit.elapsed() >= PROGRESS_THROTTLE {
                self.0.event_bus.publish(AppEvent::ObscuraDownloadProgress { downloaded_bytes, total_bytes });
                last_emit = Instant::now();
            }
        }
        file.flush().await.map_err(|err| ToolError::Install(err.to_string()))?;
        self.0.event_bus.publish(AppEvent::ObscuraDownloadProgress { downloaded_bytes, total_bytes });
        Ok(format!("{:x}", hasher.finalize()))
    }
}

/// Extracts only the files named in `FILES`, by bare file name (an entry with a path can't escape `dir`).
fn unpack(zip_path: &Path, dir: &Path) -> Result<(), ToolError> {
    let fail = |msg: String| ToolError::Install(msg);
    let file = std::fs::File::open(zip_path).map_err(|err| fail(err.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|err| fail(err.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| fail(err.to_string()))?;
        let Some(name) = Path::new(entry.name()).file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        if entry.is_dir() || !FILES.contains(&name.as_str()) {
            continue;
        }
        let mut out = std::fs::File::create(dir.join(&name)).map_err(|err| fail(err.to_string()))?;
        std::io::copy(&mut entry, &mut out).map_err(|err| fail(err.to_string()))?;
    }
    if FILES.iter().all(|name| dir.join(name).is_file()) {
        Ok(())
    } else {
        Err(fail("the archive does not contain the expected files".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn manager(dir: &Path) -> ObscuraManager {
        ObscuraManager::new(dir.to_path_buf(), EventBus::new())
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            for (name, bytes) in entries {
                writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                writer.write_all(bytes).unwrap();
            }
            writer.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn not_installed_until_both_executables_exist() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        assert!(!manager.is_installed());
        std::fs::write(dir.path().join("obscura.exe"), b"x").unwrap();
        assert!(!manager.is_installed());
        std::fs::write(dir.path().join("obscura-worker.exe"), b"x").unwrap();
        assert!(manager.is_installed());
    }

    #[test]
    fn unpack_keeps_only_the_two_executables_and_ignores_paths() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("a.zip");
        std::fs::write(&zip_path, zip_with(&[("obscura.exe", b"1"), ("obscura-worker.exe", b"2"), ("../evil.dll", b"3"), ("README.md", b"4")])).unwrap();
        let target = dir.path().join("out");
        std::fs::create_dir(&target).unwrap();

        unpack(&zip_path, &target).unwrap();

        let mut names: Vec<_> = std::fs::read_dir(&target).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["obscura-worker.exe", "obscura.exe"]);
        assert!(!dir.path().join("evil.dll").exists());
    }

    #[test]
    fn unpack_fails_when_an_executable_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("a.zip");
        std::fs::write(&zip_path, zip_with(&[("obscura.exe", b"1")])).unwrap();
        assert!(unpack(&zip_path, dir.path()).is_err());
    }

    /// Hits GitHub: `cargo test -p aivt-provider-tools downloads_and_installs -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn downloads_and_installs_the_real_release() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        manager.download().await.unwrap();
        assert!(manager.is_installed());
        assert!(!dir.path().join("obscura.zip.part").exists());
        manager.delete().await.unwrap();
        assert!(!manager.is_installed());
    }

    #[tokio::test]
    async fn delete_is_a_no_op_when_nothing_is_installed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(manager(dir.path()).delete().await.is_ok());
    }
}
