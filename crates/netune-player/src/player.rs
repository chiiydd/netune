//! Audio player implementation using rodio.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player as RodioPlayer, Source};

use netune_core::error::NetuneError;
use netune_core::Result;
use netune_core::traits::AudioPlayer;

/// Holds the rodio playback state for a single track.
struct PlaybackState {
    /// Device sink (must stay alive while playing).
    _device_sink: MixerDeviceSink,
    /// Rodio player handle — pause / resume / seek / volume control.
    player: RodioPlayer,
    /// Total duration reported by the decoder (seconds).
    duration: f64,
}

/// rodio-based audio player with streaming support.
pub struct NetunePlayer {
    /// Current playback state — `None` when stopped or not yet started.
    state: Arc<Mutex<Option<PlaybackState>>>,
}

impl NetunePlayer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for NetunePlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioPlayer for NetunePlayer {
    async fn play(&self, url: &str) -> Result<()> {
        // Download the audio data from the URL.
        let bytes = reqwest::get(url)
            .await
            .map_err(|e| NetuneError::Player(format!("Failed to fetch audio URL: {e}")))?
            .bytes()
            .await
            .map_err(|e| NetuneError::Player(format!("Failed to read audio body: {e}")))?;

        // Create decoder from the downloaded bytes.
        let cursor = Cursor::new(bytes.to_vec());
        let decoder = Decoder::new(cursor)
            .map_err(|e| NetuneError::Player(format!("Failed to decode audio: {e}")))?;

        // Read total duration before consuming the decoder.
        let duration = decoder
            .total_duration()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        // Create a device sink to the default audio output.
        let device_sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| NetuneError::Player(format!("Failed to open audio device: {e}")))?;

        // Create a player connected to the mixer.
        let rodio_player = RodioPlayer::connect_new(device_sink.mixer());

        // Apply the previously-set volume (if any).
        let prev_volume = self
            .state
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|ps| ps.player.volume()));
        if let Some(vol) = prev_volume {
            rodio_player.set_volume(vol);
        }

        // Append the decoder source — starts playback immediately.
        rodio_player.append(decoder);

        // Store the new playback state.
        {
            let mut state = self.state.lock().map_err(|e| {
                NetuneError::Player(format!("Failed to acquire player state lock: {e}"))
            })?;
            *state = Some(PlaybackState {
                _device_sink: device_sink,
                player: rodio_player,
                duration,
            });
        }

        tracing::debug!(url, duration, "Playback started");
        Ok(())
    }

    fn pause(&self) {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            ps.player.pause();
        }
    }

    fn resume(&self) {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            ps.player.play(); // rodio `play()` = resume
        }
    }

    fn toggle_pause(&self) {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            if ps.player.is_paused() {
                ps.player.play();
            } else {
                ps.player.pause();
            }
        }
    }

    fn stop(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(ref ps) = *state {
                ps.player.stop();
            }
            *state = None;
        }
    }

    fn seek(&self, seconds: f64) -> Result<()> {
        let state = self.state.lock().map_err(|e| {
            NetuneError::Player(format!("Failed to acquire player state lock: {e}"))
        })?;
        if let Some(ref ps) = *state {
            let pos = Duration::from_secs_f64(seconds.max(0.0));
            ps.player
                .try_seek(pos)
                .map_err(|e| NetuneError::Player(format!("Seek failed: {e}")))?;
        }
        Ok(())
    }

    fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            ps.player.set_volume(clamped);
        }
    }

    fn volume(&self) -> f32 {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            return ps.player.volume();
        }
        1.0
    }

    fn position(&self) -> f64 {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            return ps.player.get_pos().as_secs_f64();
        }
        0.0
    }

    fn duration(&self) -> f64 {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            return ps.duration;
        }
        0.0
    }

    fn is_playing(&self) -> bool {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            return !ps.player.is_paused();
        }
        false
    }

    fn is_paused(&self) -> bool {
        if let Ok(state) = self.state.lock()
            && let Some(ref ps) = *state
        {
            return ps.player.is_paused();
        }
        false
    }
}
