/// Media file management for Toaster.
///
/// Handles media import, validation, and metadata extraction.
/// Actual video/audio playback uses the frontend HTML5 <video> element
/// served via Tauri's `asset:` protocol.
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Supported media file extensions.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", "ogv",
];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "aac", "m4a", "wma", "opus"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum MediaType {
    Video,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct MediaInfo {
    /// Absolute path to the media file.
    pub path: PathBuf,
    /// File name without directory.
    pub file_name: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Whether this is video or audio.
    pub media_type: MediaType,
    /// File extension (lowercase).
    pub extension: String,
}

/// Cached decoded audio for the currently loaded media file.
///
/// Populated on demand by `commands::disfluency::decode_media_audio_cached`
/// and reused across repeated cleanup / smart-duplicate calls. Keyed by
/// file path + modified-time so edits to the source file on disk force a
/// re-decode, and cleared outright on `import()` / `clear()`.
///
/// `samples` is wrapped in `Arc` so callers can hold a reference without
/// keeping the `MediaState` mutex locked during their work.
#[derive(Clone)]
pub struct AudioCache {
    pub path: PathBuf,
    pub modified: SystemTime,
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
}

/// Manages the currently loaded media file.
pub struct MediaState {
    current: Option<MediaInfo>,
    audio_cache: Option<AudioCache>,
}

impl Default for MediaState {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaState {
    pub fn new() -> Self {
        Self {
            current: None,
            audio_cache: None,
        }
    }

    /// Import a media file. Validates it exists and has a supported extension.
    pub fn import(&mut self, path: &Path) -> Result<MediaInfo, String> {
        if !path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .ok_or_else(|| "File has no extension".to_string())?;

        let media_type = if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
            MediaType::Video
        } else if AUDIO_EXTENSIONS.contains(&extension.as_str()) {
            MediaType::Audio
        } else {
            return Err(format!(
                "Unsupported format '.{}'. Supported: {}",
                extension,
                VIDEO_EXTENSIONS
                    .iter()
                    .chain(AUDIO_EXTENSIONS.iter())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };

        let metadata =
            std::fs::metadata(path).map_err(|e| format!("Cannot read file metadata: {}", e))?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let info = MediaInfo {
            path: path.to_path_buf(),
            file_name,
            file_size: metadata.len(),
            media_type,
            extension,
        };

        info!(
            "Imported media: {} ({:?}, {} bytes)",
            info.file_name, info.media_type, info.file_size
        );
        self.current = Some(info.clone());
        // Invalidate the decoded-audio cache: we might be loading a
        // different file, or the same path after the user edited it
        // on disk, or any other reason the previous samples are stale.
        self.audio_cache = None;
        Ok(info)
    }

    /// Get the currently loaded media info.
    pub fn current(&self) -> Option<&MediaInfo> {
        self.current.as_ref()
    }

    /// Clear the current media.
    pub fn clear(&mut self) {
        if let Some(ref info) = self.current {
            info!("Cleared media: {}", info.file_name);
        }
        self.current = None;
        self.audio_cache = None;
    }

    /// Look up a cached decoded audio buffer for `path` with matching
    /// modified-time. Returns `None` on cache miss (different path, stale
    /// mtime, or never populated). Cheap to call — does no I/O.
    pub fn audio_cache_get(&self, path: &Path, modified: SystemTime) -> Option<(Arc<Vec<f32>>, u32)> {
        let cache = self.audio_cache.as_ref()?;
        if cache.path == path && cache.modified == modified {
            Some((cache.samples.clone(), cache.sample_rate))
        } else {
            None
        }
    }

    /// Store a freshly decoded audio buffer in the cache. Overwrites any
    /// previous entry; callers should only populate after a verified
    /// decode success.
    pub fn audio_cache_put(
        &mut self,
        path: PathBuf,
        modified: SystemTime,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
    ) {
        self.audio_cache = Some(AudioCache {
            path,
            modified,
            samples,
            sample_rate,
        });
    }

    /// Get the asset protocol URL for the current media file.
    /// This URL can be used in the frontend <video> or <audio> element.
    pub fn asset_url(&self) -> Option<String> {
        self.current.as_ref().map(|info| {
            // Tauri asset protocol: asset://localhost/<encoded-path>
            let path_str = info.path.to_string_lossy().replace('\\', "/");
            format!("asset://localhost/{}", urlencoding(&path_str))
        })
    }
}

/// Minimal percent-encoding for asset protocol paths.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for ch in s.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            // Keep forward slashes, letters, digits, and common safe chars
            '/' | '-' | '_' | '.' | ':' | '~' => out.push(ch),
            c if c.is_ascii_alphanumeric() => out.push(c),
            c => {
                for byte in c.to_string().as_bytes() {
                    out.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    out
}

/// Wrapper for Tauri managed state.
pub struct MediaStore(pub Mutex<MediaState>);

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, content: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join("toaster_media_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
    }

    #[test]
    fn import_video_file() {
        let path = temp_file("test.mp4", b"fake mp4 data");
        let mut state = MediaState::new();
        let info = state.import(&path).unwrap();
        assert_eq!(info.media_type, MediaType::Video);
        assert_eq!(info.extension, "mp4");
        assert_eq!(info.file_name, "test.mp4");
        assert!(info.file_size > 0);
        assert!(state.current().is_some());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn import_audio_file() {
        let path = temp_file("test.wav", b"fake wav data");
        let mut state = MediaState::new();
        let info = state.import(&path).unwrap();
        assert_eq!(info.media_type, MediaType::Audio);
        assert_eq!(info.extension, "wav");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn import_unsupported_extension() {
        let path = temp_file("test.xyz", b"data");
        let mut state = MediaState::new();
        let result = state.import(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported format"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn import_nonexistent_file() {
        let mut state = MediaState::new();
        let result = state.import(Path::new("C:\\nonexistent\\file.mp4"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("File not found"));
    }

    #[test]
    fn import_no_extension() {
        let path = temp_file("noext", b"data");
        let mut state = MediaState::new();
        let result = state.import(&path);
        assert!(result.is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn clear_removes_current() {
        let path = temp_file("clear_test.mp4", b"data");
        let mut state = MediaState::new();
        state.import(&path).unwrap();
        assert!(state.current().is_some());
        state.clear();
        assert!(state.current().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn asset_url_generated() {
        let path = temp_file("url_test.mp4", b"data");
        let mut state = MediaState::new();
        state.import(&path).unwrap();
        let url = state.asset_url().unwrap();
        assert!(url.starts_with("asset://localhost/"));
        assert!(url.contains("url_test.mp4"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn asset_url_none_when_empty() {
        let state = MediaState::new();
        assert!(state.asset_url().is_none());
    }

    #[test]
    fn urlencoding_spaces() {
        assert_eq!(urlencoding("my file.mp4"), "my%20file.mp4");
    }

    #[test]
    fn urlencoding_preserves_slashes() {
        assert_eq!(urlencoding("C:/path/to/file.mp4"), "C:/path/to/file.mp4");
    }

    #[test]
    fn audio_cache_miss_when_empty() {
        let state = MediaState::new();
        assert!(state
            .audio_cache_get(Path::new("/nowhere.mp4"), SystemTime::UNIX_EPOCH)
            .is_none());
    }

    #[test]
    fn audio_cache_hit_on_exact_path_and_mtime() {
        let mut state = MediaState::new();
        let path = PathBuf::from("/tmp/foo.mp4");
        let mtime = SystemTime::UNIX_EPOCH;
        let samples = Arc::new(vec![0.1f32, 0.2, 0.3]);
        state.audio_cache_put(path.clone(), mtime, samples.clone(), 16_000);

        let hit = state.audio_cache_get(&path, mtime).expect("cache hit");
        assert_eq!(hit.1, 16_000);
        assert!(Arc::ptr_eq(&hit.0, &samples));
    }

    #[test]
    fn audio_cache_miss_on_stale_mtime() {
        let mut state = MediaState::new();
        let path = PathBuf::from("/tmp/foo.mp4");
        let old_mtime = SystemTime::UNIX_EPOCH;
        let new_mtime = old_mtime + std::time::Duration::from_secs(60);
        state.audio_cache_put(path.clone(), old_mtime, Arc::new(vec![0.0f32]), 16_000);

        assert!(state.audio_cache_get(&path, new_mtime).is_none());
    }

    #[test]
    fn audio_cache_invalidated_on_import() {
        let path = temp_file("cache_invalidate.mp4", b"data");
        let mut state = MediaState::new();
        state.audio_cache_put(
            PathBuf::from("/tmp/stale.mp4"),
            SystemTime::UNIX_EPOCH,
            Arc::new(vec![1.0f32]),
            16_000,
        );
        state.import(&path).unwrap();
        assert!(state
            .audio_cache_get(Path::new("/tmp/stale.mp4"), SystemTime::UNIX_EPOCH)
            .is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn audio_cache_invalidated_on_clear() {
        let mut state = MediaState::new();
        let path = PathBuf::from("/tmp/foo.mp4");
        let mtime = SystemTime::UNIX_EPOCH;
        state.audio_cache_put(path.clone(), mtime, Arc::new(vec![0.5f32]), 16_000);
        assert!(state.audio_cache_get(&path, mtime).is_some());
        state.clear();
        assert!(state.audio_cache_get(&path, mtime).is_none());
    }
}
