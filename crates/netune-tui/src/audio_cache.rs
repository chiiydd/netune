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
        let mut cache = Self {
            dir,
            max_bytes: DEFAULT_MAX_CACHE_BYTES,
            index: HashMap::new(),
        };
        cache.scan_existing();
        cache
    }

    /// Scan existing files in the cache directory to rebuild the index.
    fn scan_existing(&mut self) {
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
    pub fn get(&mut self, song_id: u64) -> Option<Vec<u8>> {
        let entry = self.index.get_mut(&song_id)?;
        let bytes = std::fs::read(&entry.path).ok()?;
        entry.accessed = SystemTime::now();
        // Update access time on disk too.
        filetime::set_file_mtime(
            &entry.path,
            filetime::FileTime::from_system_time(entry.accessed),
        )
        .ok();
        Some(bytes)
    }

    /// Store audio bytes for a song.
    pub fn put(&mut self, song_id: u64, bytes: &[u8]) {
        let path = self.dir.join(format!("{song_id}.mp3"));
        if let Err(e) = std::fs::write(&path, bytes) {
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
        let mut entries: Vec<(u64, SystemTime)> = self
            .index
            .iter()
            .map(|(id, e)| (*id, e.accessed))
            .collect();
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
