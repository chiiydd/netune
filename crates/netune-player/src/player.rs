//! Audio player implementation using rodio.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use async_trait::async_trait;
use tokio::sync::RwLock;

use netune_core::Result;
use netune_core::traits::AudioPlayer;

/// rodio-based audio player with streaming support.
pub struct NetunePlayer {
    volume: Arc<AtomicU32>,     // stored as f32 bits
    playing: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    position: Arc<RwLock<f64>>,
    duration: Arc<RwLock<f64>>,
}

impl NetunePlayer {
    pub fn new() -> Self {
        Self {
            volume: Arc::new(AtomicU32::new(0.7f32.to_bits())),
            playing: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            position: Arc::new(RwLock::new(0.0)),
            duration: Arc::new(RwLock::new(0.0)),
        }
    }
}

#[async_trait]
impl AudioPlayer for NetunePlayer {
    async fn play(&self, _url: &str) -> Result<()> {
        todo!("A组(Codex): 实现 rodio 流式播放")
    }

    fn pause(&self) {
        todo!("A组(Codex): 实现暂停")
    }

    fn resume(&self) {
        todo!("A组(Codex): 实现恢复")
    }

    fn toggle_pause(&self) {
        if self.paused.load(Ordering::Relaxed) {
            self.resume();
        } else {
            self.pause();
        }
    }

    fn stop(&self) {
        self.playing.store(false, Ordering::Relaxed);
        self.paused.store(false, Ordering::Relaxed);
    }

    fn seek(&self, _seconds: f64) -> Result<()> {
        todo!("A组(Codex): 实现跳转")
    }

    fn set_volume(&self, volume: f32) {
        self.volume.store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    fn position(&self) -> f64 {
        // NOTE: Real impl will use tokio::spawn to track position
        0.0
    }

    fn duration(&self) -> f64 {
        0.0
    }

    fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
}
