use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayState {
    Empty,
    Playing,
    Paused,
    Ended,
    Error,
}

#[derive(Clone, Debug)]
pub struct PlaybackSnapshot {
    pub recording_id: Option<String>,
    pub state: PlayState,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub speed: f32,
    pub volume: f32,
    pub error: Option<String>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            recording_id: None,
            state: PlayState::Empty,
            position_ms: 0,
            duration_ms: 0,
            speed: 1.0,
            volume: 1.0,
            error: None,
        }
    }
}

struct Loaded {
    recording_id: String,
    path: PathBuf,
    duration_ms: u64,
}

pub struct Playback {
    sink: MixerDeviceSink,
    player: Option<Player>,
    loaded: Option<Loaded>,
    state: PlayState,
    speed: f32,
    volume: f32,
    error: Option<String>,
}

impl Playback {
    pub fn new(speed: f32, volume: f32) -> std::result::Result<Self, String> {
        let sink =
            DeviceSinkBuilder::open_default_sink().map_err(|e| format!("no audio output: {e}"))?;
        Ok(Self {
            sink,
            player: None,
            loaded: None,
            state: PlayState::Empty,
            speed: speed.clamp(0.5, 2.0),
            volume: volume.clamp(0.0, 1.0),
            error: None,
        })
    }

    pub fn load(
        &mut self,
        recording_id: &str,
        path: &Path,
        duration_ms: u64,
    ) -> std::result::Result<(), String> {
        self.open(recording_id, path, duration_ms, true)
    }

    fn open(
        &mut self,
        recording_id: &str,
        path: &Path,
        duration_ms: u64,
        autoplay: bool,
    ) -> std::result::Result<(), String> {
        let file = File::open(path).map_err(|e| format!("could not open recording: {e}"))?;
        let decoder =
            Decoder::try_from(file).map_err(|e| format!("could not decode audio: {e}"))?;

        let player = Player::connect_new(self.sink.mixer());
        player.set_volume(self.volume);
        player.set_speed(self.speed);
        player.append(decoder);

        self.player = Some(player);
        self.loaded = Some(Loaded {
            recording_id: recording_id.to_string(),
            path: path.to_path_buf(),
            duration_ms,
        });
        if autoplay {
            self.play();
        } else {
            self.pause();
        }
        self.error = None;
        Ok(())
    }

    fn play(&mut self) {
        if let Some(player) = &self.player {
            player.play();
            self.state = PlayState::Playing;
        }
    }

    fn pause(&mut self) {
        if let Some(player) = &self.player {
            player.pause();
            self.state = PlayState::Paused;
        }
    }

    pub fn toggle(&mut self) -> std::result::Result<(), String> {
        if self.state == PlayState::Ended {
            let Some(loaded) = &self.loaded else {
                return Ok(());
            };
            let id = loaded.recording_id.clone();
            let path = loaded.path.clone();
            let duration = loaded.duration_ms;
            return self.load(&id, &path, duration);
        }
        let Some(player) = &self.player else {
            return Ok(());
        };
        if self.state == PlayState::Playing {
            player.pause();
            self.state = PlayState::Paused;
        } else if self.state == PlayState::Paused {
            player.play();
            self.state = PlayState::Playing;
        }
        Ok(())
    }

    pub fn seek_to(&mut self, position_ms: u64) {
        let Some((id, path, duration)) = self.loaded.as_ref().map(|loaded| {
            (
                loaded.recording_id.clone(),
                loaded.path.clone(),
                loaded.duration_ms,
            )
        }) else {
            return;
        };
        let target = Duration::from_millis(position_ms.min(duration));
        let resume = self.state != PlayState::Paused;
        let source_is_gone = self.state == PlayState::Ended
            || self.player.as_ref().is_none_or(|player| player.empty());

        if source_is_gone {
            if self.open(&id, &path, duration, false).is_err() {
                return;
            }
            if let Some(player) = &self.player
                && player.try_seek(target).is_err()
            {
                self.error = Some("This recording could not be seeked.".to_string());
                return;
            }
            if resume {
                self.play();
            }
            return;
        }

        let Some(player) = &self.player else {
            return;
        };
        match player.try_seek(target) {
            Ok(()) => {
                if resume {
                    self.play();
                } else {
                    self.pause();
                }
            }
            Err(err) => {
                self.error = Some(format!("This recording could not be seeked: {err}"));
            }
        }
    }

    pub fn seek_relative(&mut self, delta_ms: i64) {
        let Some(loaded) = &self.loaded else {
            return;
        };
        let position = self
            .player
            .as_ref()
            .map(|player| player.get_pos().as_millis() as u64)
            .unwrap_or(0);
        let target = (position as i64 + delta_ms).clamp(0, loaded.duration_ms as i64) as u64;
        self.seek_to(target);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.5, 2.0);
        if let Some(player) = &self.player {
            player.set_speed(self.speed);
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(player) = &self.player {
            player.set_volume(self.volume);
        }
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn snapshot(&mut self) -> PlaybackSnapshot {
        let Some(loaded) = &self.loaded else {
            return PlaybackSnapshot {
                state: self.state,
                speed: self.speed,
                volume: self.volume,
                error: self.error.clone(),
                ..Default::default()
            };
        };
        let mut state = self.state;
        let mut position_ms = 0;
        if let Some(player) = &self.player {
            if state == PlayState::Playing && player.empty() {
                state = PlayState::Ended;
                self.state = PlayState::Ended;
            }
            position_ms = player.get_pos().as_millis() as u64;
        }
        position_ms = position_ms.min(loaded.duration_ms);
        PlaybackSnapshot {
            recording_id: Some(loaded.recording_id.clone()),
            state,
            position_ms,
            duration_ms: loaded.duration_ms,
            speed: self.speed,
            volume: self.volume,
            error: self.error.clone(),
        }
    }

    pub fn stop(&mut self) {
        self.player = None;
        self.loaded = None;
        self.state = PlayState::Empty;
    }
}
