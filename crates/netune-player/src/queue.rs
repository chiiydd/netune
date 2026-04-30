//! Playback queue management.

use netune_core::models::Song;

/// Play mode for the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    /// Play in order.
    Sequential,
    /// Loop the entire queue.
    LoopAll,
    /// Loop single song.
    LoopOne,
    /// Shuffle.
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

    /// Load songs into the queue.
    pub fn load(&mut self, songs: Vec<Song>) {
        self.songs = songs;
        self.current = 0;
        self.history.clear();
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

    /// Next song based on play mode.
    pub fn next(&mut self) -> Option<&Song> {
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

    /// Previous song.
    pub fn prev(&mut self) -> Option<&Song> {
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

    /// Cycle play mode: Sequential -> LoopAll -> LoopOne -> Shuffle.
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            PlayMode::Sequential => PlayMode::LoopAll,
            PlayMode::LoopAll => PlayMode::LoopOne,
            PlayMode::LoopOne => PlayMode::Shuffle,
            PlayMode::Shuffle => PlayMode::Sequential,
        };
    }
}
