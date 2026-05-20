//! Audio player implementation using rodio.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
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
#[derive(Clone)]
pub struct NetunePlayer {
    /// Current playback state — `None` when stopped or not yet started.
    state: Arc<Mutex<Option<PlaybackState>>>,
    /// Generation counter to detect superseded playback requests.
    generation: Arc<AtomicU64>,
}

impl NetunePlayer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
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
        // Increment generation to invalidate any previous threads.
        self.generation.fetch_add(1, Ordering::SeqCst);

        // Read volume BEFORE stopping old playback.
        let prev_volume = self
            .state
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|ps| ps.player.volume()));

        // Stop and drop old playback.
        if let Ok(mut state) = self.state.lock() {
            if let Some(old) = state.take() {
                old.player.stop();
            }
        }

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
        let mut device_sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| NetuneError::Player(format!("Failed to open audio device: {e}")))?;
        device_sink.log_on_drop(false);

        // Create a player connected to the mixer.
        let rodio_player = RodioPlayer::connect_new(device_sink.mixer());

        // Apply the previously-set volume (if any).
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

    async fn play_from_bytes(&self, bytes: Vec<u8>) -> Result<()> {
        // Increment generation to invalidate any previous threads.
        let my_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // Read volume BEFORE stopping old playback.
        let prev_volume = self
            .state
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|ps| ps.player.volume()));

        // Stop old playback (fast, just sets a flag).
        if let Ok(mut state) = self.state.lock() {
            if let Some(old) = state.take() {
                old.player.stop();
            }
        }

        let state_ref = Arc::clone(&self.state);
        let gen_ref = Arc::clone(&self.generation);

        // Use spawn_blocking for heavy audio work — tasks are tied to the
        // tokio runtime so they get cancelled when superseded, preventing
        // orphaned threads from competing for the audio device.
        let handle = tokio::task::spawn_blocking(move || {
            // EARLY CHECK: bail out before heavy work if superseded.
            // This prevents multiple tasks from competing for the audio device.
            if gen_ref.load(Ordering::SeqCst) != my_gen {
                tracing::debug!("Playback superseded before decode, discarding early");
                return Ok(());
            }

            let cursor = Cursor::new(bytes);
            let decoder = Decoder::new(cursor)
                .map_err(|e| NetuneError::Player(format!("Failed to decode audio: {e}")))?;

            let duration = decoder
                .total_duration()
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            // Check again before opening audio device (expensive operation).
            if gen_ref.load(Ordering::SeqCst) != my_gen {
                tracing::debug!("Playback superseded before device open, discarding");
                return Ok(());
            }

            // Retry device open — rapid song switching can leave the device
            // temporarily unavailable from a previous task's teardown.
            let mut device_sink = None;
            for attempt in 1..=3u32 {
                match DeviceSinkBuilder::open_default_sink() {
                    Ok(mut ds) => {
                        ds.log_on_drop(false);
                        device_sink = Some(ds);
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(attempt, error = %e, "Failed to open audio device, retrying");
                        if attempt < 3 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        } else {
                            return Err(NetuneError::Player(format!(
                                "Failed to open audio device after 3 attempts: {e}"
                            )));
                        }
                    }
                }
            }
            let mut device_sink = device_sink.unwrap();

            let rodio_player = RodioPlayer::connect_new(device_sink.mixer());

            if let Some(vol) = prev_volume {
                rodio_player.set_volume(vol);
            }

            rodio_player.append(decoder);

            // Final check before writing state.
            if gen_ref.load(Ordering::SeqCst) != my_gen {
                tracing::debug!("Playback superseded after append, discarding");
                return Ok(());
            }

            let mut state = state_ref.lock().map_err(|e| {
                NetuneError::Player(format!("Failed to acquire player state lock: {e}"))
            })?;
            // Double-check after acquiring lock.
            if gen_ref.load(Ordering::SeqCst) != my_gen {
                tracing::debug!("Playback superseded after lock, discarding");
                return Ok(());
            }
            *state = Some(PlaybackState {
                _device_sink: device_sink,
                player: rodio_player,
                duration,
            });

            tracing::debug!(duration, "Playback started from cached bytes");
            Ok(())
        });

        handle
            .await
            .map_err(|e| NetuneError::Player(format!("Audio task failed: {e}")))?
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
        self.generation.fetch_add(1, Ordering::SeqCst);
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
            let current = ps.player.get_pos().as_secs_f64();
            let target = (current + seconds).max(0.0).min(ps.duration);
            let pos = Duration::from_secs_f64(target);
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
