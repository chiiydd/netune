//! Playback queue management.

use netune_core::models::Song;

/// Play mode for the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    fn next(&mut self) -> Option<Self::Item> {
        let song = self.current().cloned()?;
        PlayQueue::advance(self);
        Some(song)
    }
}
