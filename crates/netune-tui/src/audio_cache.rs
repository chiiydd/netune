//! Persistent disk-based audio cache for songs.
//!
//! Stores audio data as files in ~/.netune/audio_cache/
//! with LRU eviction when the total size exceeds the limit.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Maximum total cache size in bytes (default: 500 MB).
const DEFAULT_MAX_CACHE_BYTES: u64 = 500 * 1024 * 1024;

pub struct DiskAudioCache {
    dir: PathBuf,
    max_bytes: u64,
    /// song_id -> (file_path, size, access_time)
    index: HashMap<u64, CacheEntry>,
}

struct CacheEntry {
    path: PathBuf,
    size: u64,
    accessed: SystemTime,
}

impl DiskAudioCache {
    pub fn new() -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".netune")
            .join("audio_cache");
        std::fs::create_dir_all(&dir).ok();
        let mut cache = Self::with_dir(dir, DEFAULT_MAX_CACHE_BYTES);
        cache.scan_existing();
        cache
    }

    /// Create a cache rooted at `dir` with a custom max size, without scanning for existing files.
    pub fn with_dir(dir: PathBuf, max_bytes: u64) -> Self {
        Self {
            dir,
            max_bytes,
            index: HashMap::new(),
        }
    }

    /// Scan existing files in the cache directory to rebuild the index.
    pub fn scan_existing(&mut self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("mp3") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(song_id) = stem.parse::<u64>() {
                        if let Ok(meta) = std::fs::metadata(&path) {
                            self.index.insert(
                                song_id,
                                CacheEntry {
                                    path: path.clone(),
                                    size: meta.len(),
                                    accessed: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                                },
                            );
                        }
                    }
                }
            }
        }
        tracing::info!(songs = self.index.len(), "Disk audio cache loaded");
    }

    /// Get cached audio bytes for a song.
    /// Uses async I/O to avoid blocking the tokio runtime on large file reads.
    pub async fn get(&mut self, song_id: u64) -> Option<Vec<u8>> {
        let path = {
            let entry = self.index.get_mut(&song_id)?;
            entry.accessed = SystemTime::now();
            entry.path.clone()
        };
        // Use the index's SystemTime for LRU — no need for filetime::set_file_mtime.
        tokio::fs::read(&path).await.ok()
    }

    /// Store audio bytes for a song.
    /// Uses async I/O to avoid blocking the tokio runtime on writes.
    pub async fn put(&mut self, song_id: u64, bytes: &[u8]) {
        let path = self.dir.join(format!("{song_id}.mp3"));
        if let Err(e) = tokio::fs::write(&path, bytes).await {
            tracing::warn!(error = %e, song_id, "Failed to write audio cache");
            return;
        }
        let size = bytes.len() as u64;
        self.index.insert(
            song_id,
            CacheEntry {
                path,
                size,
                accessed: SystemTime::now(),
            },
        );
        self.evict_if_needed();
    }

    /// Store lyrics JSON for a song alongside the audio cache.
    pub async fn put_lyrics(&mut self, song_id: u64, lyrics_json: &[u8]) {
        let path = self.dir.join(format!("{song_id}.lrc"));
        if let Err(e) = tokio::fs::write(&path, lyrics_json).await {
            tracing::warn!(error = %e, song_id, "Failed to write lyrics cache");
        }
    }

    /// Get cached lyrics JSON for a song.
    pub async fn get_lyrics(&mut self, song_id: u64) -> Option<Vec<u8>> {
        let path = self.dir.join(format!("{song_id}.lrc"));
        tokio::fs::read(&path).await.ok()
    }

    /// Store cover image bytes for a song alongside the audio cache.
    pub async fn put_cover(&mut self, song_id: u64, cover_bytes: &[u8]) {
        let path = self.dir.join(format!("{song_id}.cover"));
        if let Err(e) = tokio::fs::write(&path, cover_bytes).await {
            tracing::warn!(error = %e, song_id, "Failed to write cover cache");
        }
    }

    /// Get cached cover image bytes for a song.
    pub async fn get_cover(&mut self, song_id: u64) -> Option<Vec<u8>> {
        let path = self.dir.join(format!("{song_id}.cover"));
        tokio::fs::read(&path).await.ok()
    }

    /// Check if a song is cached.
    pub fn contains(&self, song_id: u64) -> bool {
        self.index.contains_key(&song_id)
    }

    /// The cache directory path (for use in async tasks).
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }

    /// Evict least-recently-used entries until total size is under the limit.
    fn evict_if_needed(&mut self) {
        let total: u64 = self.index.values().map(|e| e.size).sum();
        if total <= self.max_bytes {
            return;
        }
        // Sort by access time (oldest first).
        let mut entries: Vec<(u64, SystemTime)> =
            self.index.iter().map(|(id, e)| (*id, e.accessed)).collect();
        entries.sort_by_key(|(_, t)| *t);

        let mut current_total = total;
        for (song_id, _) in entries {
            if current_total <= self.max_bytes {
                break;
            }
            if let Some(entry) = self.index.remove(&song_id) {
                current_total -= entry.size;
                std::fs::remove_file(&entry.path).ok();
                tracing::info!(song_id, "Evicted from audio cache");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_put_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
        cache.put(1, b"audio data").await;
        assert_eq!(cache.get(1).await.unwrap(), b"audio data");
    }

    #[tokio::test]
    async fn test_cache_contains() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
        cache.put(2, b"some audio bytes").await;
        assert!(cache.contains(2));
        assert!(!cache.contains(999));
    }

    #[tokio::test]
    async fn test_cache_get_miss() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
        assert_eq!(cache.get(42).await, None);
    }

    #[tokio::test]
    async fn test_cache_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
        cache.put(1, b"old").await;
        cache.put(1, b"new data").await;
        assert_eq!(cache.get(1).await.unwrap(), b"new data");
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), 10);
        cache.put(1, b"aaaaa").await;
        cache.put(2, b"bbbbb").await;
        cache.put(3, b"ccccc").await;
        // Total is 15 > 10, so oldest entry (song 1) should be evicted.
        assert!(!cache.contains(1));
        assert!(cache.contains(2));
        assert!(cache.contains(3));
    }

    #[tokio::test]
    async fn test_cache_eviction_lru_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), 10);
        cache.put(1, b"aaaaa").await;
        cache.put(2, b"bbbbb").await;
        // Access song 1 so it becomes recently used.
        cache.get(1).await;
        // Now put song 3 — song 2 should be evicted (least recently used).
        cache.put(3, b"ccccc").await;
        assert!(cache.contains(1));
        assert!(!cache.contains(2));
        assert!(cache.contains(3));
    }

    #[test]
    fn test_cache_scan_existing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("42.mp3"), b"cached content").unwrap();
        let mut cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
        cache.scan_existing();
        assert!(cache.contains(42));
    }

    #[test]
    fn test_cache_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskAudioCache::with_dir(dir.path().to_path_buf(), DEFAULT_MAX_CACHE_BYTES);
        assert_eq!(cache.dir(), dir.path());
    }
}
