//! Playback queue management.

use netune_core::models::Song;
use serde::{Deserialize, Serialize};

/// Play mode for the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayMode {
    /// Play in order — stop at the end of the queue.
    Sequential,
    /// Loop the entire queue.
    LoopAll,
    /// Loop single song.
    LoopOne,
    /// Shuffle (random next).
    Shuffle,
}

/// Song playback queue.
pub struct PlayQueue {
    songs: Vec<Song>,
    current: usize,
    mode: PlayMode,
    history: Vec<usize>,
}

impl PlayQueue {
    pub fn new() -> Self {
        Self {
            songs: Vec::new(),
            current: 0,
            mode: PlayMode::Sequential,
            history: Vec::new(),
        }
    }

    /// Load songs into the queue, replacing all existing content.
    pub fn load(&mut self, songs: Vec<Song>) {
        self.songs = songs;
        self.current = 0;
        self.history.clear();
    }

    /// Append a single song to the end of the queue.
    pub fn push(&mut self, song: Song) {
        self.songs.push(song);
    }

    /// Remove the song at `index`. Returns the removed song if valid.
    ///
    /// If the removed song was the current song (or before it), the current
    /// index is adjusted so it still points at the same logical position.
    pub fn remove(&mut self, index: usize) -> Option<Song> {
        if index >= self.songs.len() {
            return None;
        }
        let song = self.songs.remove(index);
        if index < self.current {
            self.current = self.current.saturating_sub(1);
        } else if index == self.current
            && self.current >= self.songs.len()
            && !self.songs.is_empty()
        {
            self.current = self.songs.len() - 1;
        }
        // Remove stale history entries that pointed at the removed index.
        self.history.retain(|h| *h != index);
        for h in &mut self.history {
            if *h > index {
                *h -= 1;
            }
        }
        Some(song)
    }

    /// Current song.
    pub fn current(&self) -> Option<&Song> {
        self.songs.get(self.current)
    }

    /// Current index.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Total songs in queue.
    pub fn len(&self) -> usize {
        self.songs.len()
    }

    /// Is queue empty?
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    /// Jump to a specific index.
    pub fn jump(&mut self, index: usize) -> Option<&Song> {
        if index < self.songs.len() {
            self.history.push(self.current);
            self.current = index;
            self.songs.get(self.current)
        } else {
            None
        }
    }

    /// Skip directly to a specific index and return that song.
    pub fn skip_to(&mut self, idx: usize) -> Option<&Song> {
        if idx < self.songs.len() {
            self.current = idx;
            self.songs.get(self.current)
        } else {
            None
        }
    }

    /// Advance to the next song based on play mode.
    pub fn advance(&mut self) -> Option<&Song> {
        if self.songs.is_empty() {
            return None;
        }
        match self.mode {
            PlayMode::Sequential => {
                if self.current + 1 < self.songs.len() {
                    self.history.push(self.current);
                    self.current += 1;
                }
            }
            PlayMode::LoopAll => {
                self.history.push(self.current);
                self.current = (self.current + 1) % self.songs.len();
            }
            PlayMode::LoopOne => {
                // Stay on current.
            }
            PlayMode::Shuffle => {
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as usize;
                self.history.push(self.current);
                self.current = seed % self.songs.len();
            }
        }
        self.songs.get(self.current)
    }

    /// Previous song (pops from history if available, otherwise decrements).
    pub fn prev(&mut self) -> Option<&Song> {
        if self.songs.is_empty() {
            return None;
        }
        if let Some(prev_idx) = self.history.pop() {
            self.current = prev_idx;
        } else if self.current > 0 {
            self.current -= 1;
        }
        self.songs.get(self.current)
    }

    /// Current play mode.
    pub fn mode(&self) -> PlayMode {
        self.mode
    }

    /// Set the play mode directly.
    pub fn set_repeat_mode(&mut self, mode: PlayMode) {
        self.mode = mode;
    }

    /// Cycle play mode: Sequential → LoopAll → LoopOne → Shuffle → Sequential.
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            PlayMode::Sequential => PlayMode::LoopAll,
            PlayMode::LoopAll => PlayMode::LoopOne,
            PlayMode::LoopOne => PlayMode::Shuffle,
            PlayMode::Shuffle => PlayMode::Sequential,
        };
    }

    /// Shuffle the queue (Fisher-Yates) while keeping the current song in place.
    pub fn shuffle(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        if self.songs.len() <= 1 {
            return;
        }
        let current_song = self.songs.remove(self.current);
        // Simple shuffle using system time as seed (no extra deps).
        let mut seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        for i in (1..self.songs.len()).rev() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (seed as usize) % (i + 1);
            self.songs.swap(i, j);
        }
        // Re-insert current song at the front.
        self.songs.insert(0, current_song);
        self.current = 0;
        self.history.clear();
    }

    /// Get a reference to all songs in the queue.
    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    /// Save the queue state to a JSON file for persistence across sessions.
    pub fn save_to_file(&self, path: &std::path::Path) -> netune_core::Result<()> {
        let snapshot = QueueSnapshot {
            songs: self.songs.clone(),
            current: self.current,
            mode: self.mode,
        };
        let json = serde_json::to_string_pretty(&snapshot)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a queue from a previously saved JSON file.
    pub fn load_from_file(path: &std::path::Path) -> netune_core::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let snapshot: QueueSnapshot = serde_json::from_str(&json)?;
        Ok(PlayQueue {
            songs: snapshot.songs,
            current: snapshot.current,
            mode: snapshot.mode,
            history: Vec::new(),
        })
    }
}

/// Serializable snapshot of the play queue for persistence.
#[derive(Serialize, Deserialize)]
struct QueueSnapshot {
    songs: Vec<Song>,
    current: usize,
    mode: PlayMode,
}

impl Default for PlayQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for PlayQueue {
    type Item = Song;

    /// Returns the current song (cloned) and advances to the next one.
    /// The advancement respects the current play mode.
    /// Returns `None` when the queue is exhausted (e.g. Sequential mode at
    /// the end of the queue).
    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.current;
        let song = self.current().cloned()?;
        PlayQueue::advance(self);
        // In Sequential mode, advance() is a no-op at the last song.
        // Detect this so the iterator terminates.
        if self.mode == PlayMode::Sequential && self.current == idx {
            self.current = self.songs.len(); // mark exhausted
            return Some(song);
        }
        Some(song)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netune_core::models::{Album, Artist, QualityLevel, Song};

    /// Helper to build a test song with a given id and name.
    fn make_song(id: u64, name: &str) -> Song {
        Song {
            id,
            name: name.to_string(),
            artists: vec![Artist {
                id: 1,
                name: "Test Artist".to_string(),
            }],
            album: Album {
                id: 1,
                name: "Test Album".to_string(),
                cover_url: None,
            },
            duration: 180_000,
            quality: QualityLevel::Standard,
        }
    }

    /// Helper: create a queue pre-loaded with 3 songs.
    fn queue_with_three() -> PlayQueue {
        let mut q = PlayQueue::new();
        q.push(make_song(1, "Song A"));
        q.push(make_song(2, "Song B"));
        q.push(make_song(3, "Song C"));
        q
    }

    #[test]
    fn test_queue_push_and_current() {
        let mut q = PlayQueue::new();
        assert!(q.current().is_none());

        q.push(make_song(1, "Song A"));
        assert_eq!(q.current().unwrap().name, "Song A");

        q.push(make_song(2, "Song B"));
        // current still points to the first song after push
        assert_eq!(q.current().unwrap().name, "Song A");
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn test_queue_advance_sequential() {
        let mut q = queue_with_three();
        q.set_repeat_mode(PlayMode::Sequential);

        assert_eq!(q.advance().unwrap().name, "Song B");
        assert_eq!(q.advance().unwrap().name, "Song C");
        // At the end — should stay on the last song.
        assert_eq!(q.advance().unwrap().name, "Song C");
        assert_eq!(q.current_index(), 2);
    }

    #[test]
    fn test_queue_advance_loop_all() {
        let mut q = queue_with_three();
        q.set_repeat_mode(PlayMode::LoopAll);

        assert_eq!(q.advance().unwrap().name, "Song B");
        assert_eq!(q.advance().unwrap().name, "Song C");
        // Wraps around to the beginning.
        assert_eq!(q.advance().unwrap().name, "Song A");
        assert_eq!(q.current_index(), 0);
    }

    #[test]
    fn test_queue_advance_loop_one() {
        let mut q = queue_with_three();
        q.set_repeat_mode(PlayMode::LoopOne);

        assert_eq!(q.advance().unwrap().name, "Song A");
        assert_eq!(q.advance().unwrap().name, "Song A");
        assert_eq!(q.advance().unwrap().name, "Song A");
        assert_eq!(q.current_index(), 0);
    }

    #[test]
    fn test_queue_prev() {
        let mut q = queue_with_three();

        // advance twice → Song C
        q.advance(); // B
        q.advance(); // C
        assert_eq!(q.current().unwrap().name, "Song C");

        // prev uses history → back to B
        assert_eq!(q.prev().unwrap().name, "Song B");
        // prev again → back to A
        assert_eq!(q.prev().unwrap().name, "Song A");
        // prev at beginning → stays on A (no more history, index 0)
        assert_eq!(q.prev().unwrap().name, "Song A");
    }

    #[test]
    fn test_queue_remove() {
        let mut q = queue_with_three();
        // Remove middle song.
        let removed = q.remove(1).unwrap();
        assert_eq!(removed.name, "Song B");
        assert_eq!(q.len(), 2);
        // Current is still index 0 → Song A.
        assert_eq!(q.current().unwrap().name, "Song A");

        // Advance → should go to Song C (the remaining song at index 1).
        assert_eq!(q.advance().unwrap().name, "Song C");

        // Remove out-of-bounds returns None.
        assert!(q.remove(99).is_none());
    }

    #[test]
    fn test_queue_remove_current_adjusts() {
        let mut q = queue_with_three();
        q.advance(); // now at index 1 (Song B)

        // Remove current song (index 1). Current song was Song B,
        // after removal Song C shifts to index 1.
        let removed = q.remove(1).unwrap();
        assert_eq!(removed.name, "Song B");
        assert_eq!(q.current().unwrap().name, "Song C");
    }

    #[test]
    fn test_queue_shuffle() {
        let mut q = PlayQueue::new();
        for i in 0..10 {
            q.push(make_song(i, &format!("Song {i}")));
        }
        let original_names: Vec<String> = q.songs().iter().map(|s| s.name.clone()).collect();

        q.shuffle();

        // Current song should remain the same (Song 0).
        assert_eq!(q.current().unwrap().name, "Song 0");
        // Queue length unchanged.
        assert_eq!(q.len(), 10);
        // All songs still present (order may differ).
        let mut shuffled_names: Vec<String> = q.songs().iter().map(|s| s.name.clone()).collect();
        let mut sorted_orig = original_names.clone();
        sorted_orig.sort();
        shuffled_names.sort();
        assert_eq!(sorted_orig, shuffled_names);
    }

    #[test]
    fn test_queue_empty() {
        let mut q = PlayQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.current().is_none());
        assert!(q.advance().is_none());
        assert!(q.prev().is_none());
        assert!(q.remove(0).is_none());
    }

    #[test]
    fn test_queue_iterator() {
        let mut q = PlayQueue::new();
        q.push(make_song(1, "Song A"));
        q.push(make_song(2, "Song B"));
        q.push(make_song(3, "Song C"));
        // Sequential mode: iterator yields each song then advances.
        let collected: Vec<String> = q.by_ref().map(|s| s.name).collect();
        assert_eq!(collected, vec!["Song A", "Song B", "Song C"]);
    }

    #[test]
    fn test_queue_iterator_loop_all_yields_forever() {
        let mut q = queue_with_three();
        q.set_repeat_mode(PlayMode::LoopAll);

        // Take 6 items — should cycle through twice.
        let names: Vec<String> = q.by_ref().take(6).map(|s| s.name).collect();
        assert_eq!(
            names,
            vec!["Song A", "Song B", "Song C", "Song A", "Song B", "Song C"]
        );
    }

    #[test]
    fn test_queue_cycle_mode() {
        let mut q = PlayQueue::new();
        assert_eq!(q.mode(), PlayMode::Sequential);
        q.cycle_mode();
        assert_eq!(q.mode(), PlayMode::LoopAll);
        q.cycle_mode();
        assert_eq!(q.mode(), PlayMode::LoopOne);
        q.cycle_mode();
        assert_eq!(q.mode(), PlayMode::Shuffle);
        q.cycle_mode();
        assert_eq!(q.mode(), PlayMode::Sequential);
    }

    #[test]
    fn test_queue_jump() {
        let mut q = queue_with_three();
        let song = q.jump(2).unwrap();
        assert_eq!(song.name, "Song C");
        assert_eq!(q.current_index(), 2);

        // jump out of bounds
        assert!(q.jump(99).is_none());
    }

    #[test]
    fn test_queue_load() {
        let mut q = queue_with_three();
        q.advance(); // move to index 1

        let new_songs = vec![make_song(10, "New A"), make_song(11, "New B")];
        q.load(new_songs);

        assert_eq!(q.len(), 2);
        assert_eq!(q.current().unwrap().name, "New A");
        assert_eq!(q.current_index(), 0);
    }
}
