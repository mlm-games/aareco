use std::path::{Path, PathBuf};

use game_utils::save_store::SaveStore;
use game_utils::storage::FsStorage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaveformStyle {
    #[default]
    Bars,
    Line,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
    Title,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartTab {
    #[default]
    Record,
    Library,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default = "default_true")]
    pub dark: bool,
    #[serde(default = "default_speed")]
    pub playback_speed: f32,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub recent_topic: Option<String>,
    #[serde(default)]
    pub new_recording_topic: Option<String>,
    #[serde(default = "default_true")]
    pub confirm_save: bool,
    #[serde(default)]
    pub waveform_style: WaveformStyle,
    #[serde(default)]
    pub sort: SortOrder,
    #[serde(default)]
    pub start_tab: StartTab,
    #[serde(default)]
    pub last_backup_at: Option<i64>,
    #[serde(default = "default_scrub_back")]
    pub scrub_back_secs: u32,
    #[serde(default = "default_scrub_forward")]
    pub scrub_forward_secs: u32,
    #[serde(default = "default_mark_rewind")]
    pub mark_rewind_secs: u32,
    #[serde(default = "default_mark_nudge")]
    pub mark_nudge_secs: u32,
    #[serde(default)]
    pub plugin_paths: Vec<String>,
    #[serde(default)]
    pub default_template: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_speed() -> f32 {
    1.0
}

fn default_volume() -> f32 {
    1.0
}

fn default_scrub_back() -> u32 {
    15
}

fn default_scrub_forward() -> u32 {
    15
}

fn default_mark_rewind() -> u32 {
    2
}

fn default_mark_nudge() -> u32 {
    5
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device: None,
            sample_rate: None,
            dark: true,
            playback_speed: 1.0,
            volume: 1.0,
            recent_topic: None,
            new_recording_topic: None,
            confirm_save: true,
            waveform_style: WaveformStyle::Bars,
            sort: SortOrder::Newest,
            start_tab: StartTab::Record,
            last_backup_at: None,
            scrub_back_secs: 15,
            scrub_forward_secs: 15,
            mark_rewind_secs: 2,
            mark_nudge_secs: 5,
            plugin_paths: Vec::new(),
            default_template: None,
        }
    }
}

impl Settings {
    pub fn load(root: &Path) -> Self {
        let file = SettingsFile::new(root, "settings.json");
        let Some(bytes) = file.load(&SettingsFile::is_intact_json, &[]).data else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!(
                "ignoring settings at {}: {e}",
                settings_path(root).display()
            );
            Self::default()
        })
    }

    pub fn save(&self, root: &Path) {
        let Ok(json) = serde_json::to_vec_pretty(self) else {
            return;
        };
        if let Err(e) = SettingsFile::new(root, "settings.json").write(&json) {
            log::warn!(
                "could not save settings at {}: {e}",
                settings_path(root).display()
            );
        }
    }
}

type SettingsFile = SaveStore<FsStorage>;

pub fn settings_path(root: &Path) -> PathBuf {
    root.join("settings.json")
}
