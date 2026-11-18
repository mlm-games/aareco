use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use aareco_fx::{EffectInfo, FxChain};
use aareco_recorder::{
    CaptureConfig, DeviceInfo, Mark as RecorderMark, Recorder, RecorderSnapshot, StopResult,
    downsample_peaks, input_devices, recover_partial_files, wav,
};
use aareco_store::{FxParam, FxSlot, Library, Mark, Recording, Store, WAVEFORM_POINTS, now_ms};
use chrono::Local;
use repose_core::prelude::*;
use repose_material::material3::{DialogState, show_simple_snackbar};
use repose_navigation::Navigator;
use repose_ui::overlay::SnackbarController;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::playback::{PlayState, Playback, PlaybackSnapshot};
use crate::settings::Settings;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Route {
    Home,
    Library,
    Topics,
    Settings,
    Record,
    Player { id: String },
    Topic { id: String },
    Inbox,
}

impl Route {
    pub fn is_root(&self) -> bool {
        matches!(
            self,
            Route::Home | Route::Library | Route::Topics | Route::Settings
        )
    }

    pub fn tab_index(&self) -> usize {
        match self {
            Route::Home | Route::Record => 0,
            Route::Library | Route::Player { .. } => 1,
            Route::Topics | Route::Topic { .. } | Route::Inbox => 2,
            Route::Settings => 3,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub started_at: i64,
    pub topic_id: Option<String>,
    pub default_title: String,
}

pub struct Session {
    pub recorder: Recorder,
    pub meta: SessionMeta,
}

pub struct PendingRecording {
    pub stop: StopResult,
    pub meta: SessionMeta,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DialogKind {
    None,
    SaveRecording,
    DiscardRecording,
    RenameRecording(String),
    RenameTopic(String),
    NewTopic { parent: Option<String> },
    DeleteRecording(String),
    DeleteTopic(String),
    MoveRecording(String),
    DefaultTopic,
    Microphone,
    AddEffect,
    EffectParams(usize),
    Error(String),
}

pub struct App {
    pub nav: Navigator<Route>,
    pub root: PathBuf,
    pub store: Rc<RefCell<Store>>,
    pub library: Signal<Library>,
    pub settings: Signal<Settings>,
    pub devices: Signal<Vec<DeviceInfo>>,
    pub session: Rc<RefCell<Option<Session>>>,
    pub pending: Rc<RefCell<Option<PendingRecording>>>,
    pub playback: Rc<RefCell<Option<Playback>>>,
    pub dialog_state: Rc<DialogState>,
    pub dialog: Signal<DialogKind>,
    pub dialog_title: Signal<String>,
    pub dialog_topic: Signal<Option<String>>,
    pub snackbar: SnackbarController,
    pub search: Signal<String>,
    pub library_topic: Signal<Option<String>>,
    pub startup_warning: RefCell<Option<String>>,
    pub fx_scan: RefCell<FxScan>,
    fx_results: Arc<Mutex<Option<FxScanResult>>>,
    pub fx_render: RefCell<Option<FxRenderJob>>,
    fx_render_result: Arc<Mutex<Option<std::result::Result<(), String>>>>,
    pub fx_recording: RefCell<Option<String>>,
    pub fx_params: RefCell<Vec<yadaw_plugin_api::UnifiedParamInfo>>,
    pub fx_values: RefCell<Vec<FxParam>>,
    pub fx_search: Signal<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum FxScanStatus {
    #[default]
    Idle,
    Scanning,
    Ready,
    Failed(String),
}

#[derive(Default)]
pub struct FxScan {
    pub status: FxScanStatus,
    pub effects: Vec<EffectInfo>,
}

type FxScanResult = std::result::Result<Vec<EffectInfo>, String>;

#[derive(Clone, Debug, PartialEq)]
pub struct FxRenderJob {
    pub recording_id: String,
    pub source: String,
    pub output_name: String,
}

impl App {
    pub fn new(nav: Navigator<Route>) -> std::result::Result<Self, String> {
        let root = crate::paths::data_root();
        std::fs::create_dir_all(&root)
            .map_err(|e| format!("could not create data directory {}: {e}", root.display()))?;

        let mut startup_warning = None;
        let mut store = match Store::open(&root) {
            Ok(store) => store,
            Err(err) => {
                log::error!("{err}");
                startup_warning = Some(
                    "The library file was damaged and has been set aside. A new library was opened."
                        .to_string(),
                );
                Store::open(&root).map_err(|e| e.to_string())?
            }
        };
        adopt_existing_files(&mut store);

        let settings = Settings::load(&root);
        let devices = input_devices().unwrap_or_default();
        let playback = match Playback::new(settings.playback_speed, settings.volume) {
            Ok(player) => Some(player),
            Err(err) => {
                log::warn!("{err}");
                None
            }
        };
        let library = store.library().clone();

        Ok(Self {
            nav,
            root,
            store: Rc::new(RefCell::new(store)),
            library: signal(library),
            settings: signal(settings),
            devices: signal(devices),
            session: Rc::new(RefCell::new(None)),
            pending: Rc::new(RefCell::new(None)),
            playback: Rc::new(RefCell::new(playback)),
            dialog_state: Rc::new(DialogState::new()),
            dialog: signal(DialogKind::None),
            dialog_title: signal(String::new()),
            dialog_topic: signal(None),
            snackbar: SnackbarController::ambient(),
            search: signal(String::new()),
            library_topic: signal(None),
            startup_warning: RefCell::new(startup_warning),
            fx_scan: RefCell::new(FxScan::default()),
            fx_results: Arc::new(Mutex::new(None)),
            fx_render: RefCell::new(None),
            fx_render_result: Arc::new(Mutex::new(None)),
            fx_recording: RefCell::new(None),
            fx_params: RefCell::new(Vec::new()),
            fx_values: RefCell::new(Vec::new()),
            fx_search: signal(String::new()),
        })
    }

    pub fn current_route(&self) -> Route {
        self.nav
            .stack
            .top()
            .map(|(_, route, _, _)| route)
            .unwrap_or(Route::Home)
    }

    pub fn refresh(&self) {
        let library = self.store.borrow().library().clone();
        self.library.set(library);
    }

    pub fn toast(&self, message: impl Into<String>) {
        show_simple_snackbar(&self.snackbar, message.into(), None, None, 2600);
    }

    pub fn startup_warning(&self) -> Option<String> {
        self.startup_warning.borrow_mut().take()
    }

    pub fn update_settings(&self, update: impl FnOnce(&mut Settings)) {
        let mut settings = self.settings.get();
        update(&mut settings);
        settings.save(&self.root);
        self.settings.set(settings);
    }

    pub fn refresh_devices(&self) {
        match input_devices() {
            Ok(devices) => self.devices.set(devices),
            Err(err) => self.toast(format!("Could not list microphones: {err}")),
        }
    }

    pub fn session_snapshot(&self) -> Option<RecorderSnapshot> {
        self.session
            .borrow()
            .as_ref()
            .map(|session| session.recorder.snapshot())
    }

    pub fn start_recording(&self) {
        if self.session.borrow().is_some() {
            return;
        }
        let settings = self.settings.get();
        let mut topic = settings
            .new_recording_topic
            .clone()
            .or_else(|| settings.recent_topic.clone());
        if topic
            .as_deref()
            .is_some_and(|id| self.store.borrow().library().topic(id).is_none())
        {
            topic = None;
        }
        let id = Uuid::new_v4().to_string();
        let path = self
            .store
            .borrow()
            .recordings_dir()
            .join(format!("{id}.part"));
        let default_title = format!("Recording · {}", Local::now().format("%b %-d, %H:%M"));
        let config = CaptureConfig {
            device: settings.device,
            sample_rate: settings.sample_rate,
        };
        match Recorder::start(config, path) {
            Ok(recorder) => {
                *self.session.borrow_mut() = Some(Session {
                    recorder,
                    meta: SessionMeta {
                        id,
                        started_at: now_ms(),
                        topic_id: topic,
                        default_title,
                    },
                });
                self.nav.push(Route::Record);
            }
            Err(err) => {
                self.error_dialog(format!("Could not start recording: {err}"));
            }
        }
    }

    pub fn toggle_pause_recording(&self) {
        if let Some(session) = self.session.borrow().as_ref() {
            session.recorder.toggle_pause();
        }
    }

    pub fn add_mark(&self) {
        if let Some(session) = self.session.borrow().as_ref() {
            let count = session.recorder.snapshot().marks.len() + 1;
            session.recorder.mark(Some(format!("Mark {count}")));
        }
    }

    pub fn stop_recording(&self) {
        let Some(session) = self.session.borrow_mut().take() else {
            return;
        };
        let meta = session.meta.clone();
        match session.recorder.stop() {
            Ok(stop) => {
                if let Some(err) = &stop.error {
                    log::warn!("recording finished with an error: {err}");
                }
                *self.pending.borrow_mut() = Some(PendingRecording { stop, meta });
                if self.current_route() == Route::Record {
                    self.nav.pop();
                }
                if self.settings.get().confirm_save {
                    self.open_save_dialog();
                } else {
                    self.auto_save_pending();
                }
            }
            Err(err) => self.error_dialog(format!("Could not finish recording: {err}")),
        }
    }

    pub fn open_discard_dialog(&self) {
        if self.session.borrow().is_some() {
            self.open_dialog(DialogKind::DiscardRecording, String::new(), None);
        }
    }

    pub fn confirm_discard_recording(&self) {
        let Some(session) = self.session.borrow_mut().take() else {
            return;
        };
        if let Err(err) = session.recorder.cancel() {
            log::warn!("could not remove recording: {err}");
        }
        if self.current_route() == Route::Record {
            self.nav.pop();
        }
        self.close_dialog();
        self.toast("Recording discarded");
    }

    fn open_save_dialog(&self) {
        let (title, topic) = {
            let pending = self.pending.borrow();
            let Some(pending) = pending.as_ref() else {
                return;
            };
            (
                pending.meta.default_title.clone(),
                pending.meta.topic_id.clone(),
            )
        };
        self.open_dialog(DialogKind::SaveRecording, title, topic);
    }

    pub fn save_pending(&self) {
        let title = self.dialog_title.get().trim().to_string();
        if title.is_empty() {
            self.toast("Give the recording a name first");
            return;
        }
        let topic = self.dialog_topic.get();
        self.finalize_pending(title, topic);
    }

    fn auto_save_pending(&self) {
        let (title, topic) = {
            let pending = self.pending.borrow();
            let Some(pending) = pending.as_ref() else {
                return;
            };
            (
                pending.meta.default_title.clone(),
                pending.meta.topic_id.clone(),
            )
        };
        self.finalize_pending(title, topic);
    }

    fn finalize_pending(&self, title: String, topic: Option<String>) {
        let Some(pending) = self.pending.borrow_mut().take() else {
            self.close_dialog();
            return;
        };

        let mut path = pending.stop.path.clone();
        let mut duration_ms = pending.stop.duration_ms;
        let mut sample_rate = pending.stop.sample_rate;
        if path.extension().and_then(|e| e.to_str()) == Some("part") {
            match wav::recover_partial(&path) {
                Ok(Some(info)) => {
                    path = info.path.clone();
                    duration_ms = info.duration_ms();
                    sample_rate = info.sample_rate;
                }
                Ok(None) => {}
                Err(err) => log::warn!("could not recover stopped recording: {err}"),
            }
        }
        if !path.exists() {
            *self.pending.borrow_mut() = Some(pending);
            self.toast("The recording file is missing. Nothing was saved.");
            return;
        }

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.wav", pending.meta.id));
        let waveform = {
            let peaks = downsample_peaks(&pending.stop.peaks, WAVEFORM_POINTS);
            if peaks.is_empty() {
                wav::read_peaks(&path, WAVEFORM_POINTS).unwrap_or_default()
            } else {
                peaks
            }
        };
        let marks = pending
            .stop
            .marks
            .iter()
            .map(|mark: &RecorderMark| Mark {
                id: Uuid::new_v4().to_string(),
                offset_ms: mark.offset_ms,
                label: mark.label.clone(),
            })
            .collect();
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let topic = topic.filter(|id| self.store.borrow().library().topic(id).is_some());
        let recording = Recording {
            id: pending.meta.id.clone(),
            title,
            file_name,
            topic_id: topic.clone(),
            created_at: pending.meta.started_at,
            duration_ms,
            sample_rate,
            size_bytes,
            marks,
            waveform,
            source_file: None,
            fx: Vec::new(),
        };

        let result = self.store.borrow_mut().add_recording(recording);
        if let Err(err) = result {
            *self.pending.borrow_mut() = Some(pending);
            self.toast(format!("Could not save the recording: {err}"));
            return;
        }
        self.update_settings(|settings| settings.recent_topic = topic);
        self.refresh();
        self.close_dialog();
        self.toast("Recording saved");
        self.open_player(&pending.meta.id);
    }

    pub fn discard_pending(&self) {
        let Some(pending) = self.pending.borrow_mut().take() else {
            self.close_dialog();
            return;
        };
        let path = pending.stop.path;
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("part"));
        self.close_dialog();
        self.toast("Recording discarded");
    }

    pub fn playback_snapshot(&self) -> PlaybackSnapshot {
        self.playback
            .borrow_mut()
            .as_mut()
            .map(Playback::snapshot)
            .unwrap_or_default()
    }

    pub fn is_playing(&self) -> bool {
        self.playback_snapshot().state == PlayState::Playing
    }

    pub fn open_player(&self, id: &str) {
        let loaded = self.playback_snapshot().recording_id.as_deref() == Some(id);
        if !loaded && !self.load_recording(id) {
            return;
        }
        if self.current_route() != (Route::Player { id: id.to_string() }) {
            self.nav.push(Route::Player { id: id.to_string() });
        }
    }

    pub fn toggle_recording(&self, id: &str) {
        let snapshot = self.playback_snapshot();
        if snapshot.recording_id.as_deref() == Some(id) {
            if let Some(player) = self.playback.borrow_mut().as_mut() {
                if let Err(err) = player.toggle() {
                    self.toast(err);
                }
            }
        } else {
            self.load_recording(id);
        }
    }

    fn load_recording(&self, id: &str) -> bool {
        let Some(recording) = self.store.borrow().library().recording(id).cloned() else {
            self.toast("Recording not found");
            return false;
        };
        let path = self.store.borrow().recording_path(&recording);
        if !path.exists() {
            self.error_dialog("This recording's audio file is missing.".to_string());
            return false;
        }
        if self.playback.borrow().is_none() {
            let settings = self.settings.get();
            match Playback::new(settings.playback_speed, settings.volume) {
                Ok(player) => *self.playback.borrow_mut() = Some(player),
                Err(err) => {
                    self.error_dialog(err);
                    return false;
                }
            }
        }
        if let Some(player) = self.playback.borrow_mut().as_mut() {
            if let Err(err) = player.load(id, &path, recording.duration_ms) {
                self.error_dialog(err);
                return false;
            }
        }
        true
    }

    pub fn player_toggle(&self) {
        if self.playback_snapshot().recording_id.is_none()
            && let Route::Player { id } = self.current_route()
            && self.load_recording(&id)
        {
            request_frame();
            return;
        }
        if let Some(player) = self.playback.borrow_mut().as_mut()
            && let Err(err) = player.toggle()
        {
            self.toast(err);
        }
        request_frame();
    }

    pub fn player_seek(&self, position_ms: u64) {
        if let Some(player) = self.playback.borrow_mut().as_mut() {
            player.seek_to(position_ms);
        }
        request_frame();
    }

    pub fn player_seek_relative(&self, delta_ms: i64) {
        if let Some(player) = self.playback.borrow_mut().as_mut() {
            player.seek_relative(delta_ms);
        }
        request_frame();
    }

    pub fn player_set_speed(&self, speed: f32) {
        if let Some(player) = self.playback.borrow_mut().as_mut() {
            player.set_speed(speed);
        }
        self.update_settings(|settings| settings.playback_speed = speed);
    }

    pub fn player_set_volume(&self, volume: f32) {
        if let Some(player) = self.playback.borrow_mut().as_mut() {
            player.set_volume(volume);
        }
        let mut settings = self.settings.get();
        settings.volume = volume;
        self.settings.set(settings);
    }

    pub fn persist_settings(&self) {
        self.settings.get().save(&self.root);
    }

    pub fn add_mark_to_recording(&self, id: &str, offset_ms: u64) {
        let result = self.store.borrow_mut().add_mark(id, offset_ms, None);
        match result {
            Ok(_) => self.refresh(),
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn delete_mark(&self, recording_id: &str, mark_id: &str) {
        let result = self.store.borrow_mut().delete_mark(recording_id, mark_id);
        match result {
            Ok(()) => self.refresh(),
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn nudge_mark(&self, recording_id: &str, mark_id: &str, delta_secs: i32) {
        let result =
            self.store
                .borrow_mut()
                .nudge_mark(recording_id, mark_id, delta_secs as i64 * 1000);
        match result {
            Ok(()) => self.refresh(),
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn jump_to_mark(&self, recording_id: &str, forward: bool) {
        let Some(recording) = self
            .store
            .borrow()
            .library()
            .recording(recording_id)
            .cloned()
        else {
            return;
        };
        let position = if self.playback_snapshot().recording_id.as_deref() == Some(recording_id) {
            self.playback_snapshot().position_ms
        } else {
            0
        };
        let target = if forward {
            recording
                .marks
                .iter()
                .map(|mark| mark.offset_ms)
                .filter(|offset| *offset > position + 500)
                .min()
        } else {
            let threshold = self.settings.get().mark_rewind_secs as u64 * 1000;
            recording
                .marks
                .iter()
                .map(|mark| mark.offset_ms)
                .filter(|offset| offset.saturating_add(threshold) < position)
                .max()
                .or(Some(0))
        };
        if let Some(target) = target {
            self.player_seek(target);
        }
    }

    pub fn open_dialog(&self, kind: DialogKind, title: String, topic: Option<String>) {
        self.dialog.set(kind);
        self.dialog_title.set(title);
        self.dialog_topic.set(topic);
        self.dialog_state.show();
    }

    pub fn close_dialog(&self) {
        self.dialog_state.dismiss();
        self.dialog.set(DialogKind::None);
    }

    pub fn error_dialog(&self, message: impl Into<String>) {
        self.open_dialog(DialogKind::Error(message.into()), String::new(), None);
    }

    pub fn on_dialog_dismiss(&self) {
        match self.dialog.get() {
            DialogKind::SaveRecording | DialogKind::DiscardRecording => {}
            _ => self.close_dialog(),
        }
    }

    pub fn open_rename_recording(&self, id: &str) {
        let Some(recording) = self.store.borrow().library().recording(id).cloned() else {
            return;
        };
        self.open_dialog(
            DialogKind::RenameRecording(id.to_string()),
            recording.title,
            None,
        );
    }

    pub fn open_delete_recording(&self, id: &str) {
        self.open_dialog(
            DialogKind::DeleteRecording(id.to_string()),
            String::new(),
            None,
        );
    }

    pub fn open_move_recording(&self, id: &str) {
        let topic = self
            .store
            .borrow()
            .library()
            .recording(id)
            .and_then(|recording| recording.topic_id.clone());
        self.open_dialog(
            DialogKind::MoveRecording(id.to_string()),
            String::new(),
            topic,
        );
    }

    pub fn open_new_topic(&self, parent: Option<String>) {
        self.open_dialog(DialogKind::NewTopic { parent }, String::new(), None);
    }

    pub fn open_rename_topic(&self, id: &str) {
        let Some(topic) = self.store.borrow().library().topic(id).cloned() else {
            return;
        };
        self.open_dialog(DialogKind::RenameTopic(id.to_string()), topic.name, None);
    }

    pub fn open_delete_topic(&self, id: &str) {
        self.open_dialog(DialogKind::DeleteTopic(id.to_string()), String::new(), None);
    }

    pub fn open_microphone_picker(&self) {
        self.open_dialog(DialogKind::Microphone, String::new(), None);
    }

    pub fn open_default_topic(&self) {
        self.open_dialog(
            DialogKind::DefaultTopic,
            String::new(),
            self.settings.get().new_recording_topic.clone(),
        );
    }

    pub fn confirm_dialog(&self) {
        match self.dialog.get() {
            DialogKind::None => {}
            DialogKind::SaveRecording => self.save_pending(),
            DialogKind::DiscardRecording => self.confirm_discard_recording(),
            DialogKind::RenameRecording(id) => {
                let title = self.dialog_title.get();
                let result = self.store.borrow_mut().rename_recording(&id, &title);
                match result {
                    Ok(()) => {
                        self.refresh();
                        self.close_dialog();
                    }
                    Err(err) => self.toast(err.to_string()),
                }
            }
            DialogKind::RenameTopic(id) => {
                let name = self.dialog_title.get();
                let result = self.store.borrow_mut().rename_topic(&id, &name);
                match result {
                    Ok(()) => {
                        self.refresh();
                        self.close_dialog();
                    }
                    Err(err) => self.toast(err.to_string()),
                }
            }
            DialogKind::NewTopic { parent } => {
                let name = self.dialog_title.get();
                let result = self.store.borrow_mut().add_topic(&name, parent);
                match result {
                    Ok(_) => {
                        self.refresh();
                        self.close_dialog();
                    }
                    Err(err) => self.toast(err.to_string()),
                }
            }
            DialogKind::DeleteRecording(id) => {
                let current = self.playback_snapshot().recording_id;
                if current.as_deref() == Some(id.as_str()) {
                    if let Some(player) = self.playback.borrow_mut().as_mut() {
                        player.stop();
                    }
                }
                let result = self.store.borrow_mut().delete_recording(&id);
                match result {
                    Ok(()) => {
                        self.refresh();
                        self.close_dialog();
                        if self.current_route() == (Route::Player { id: id.clone() }) {
                            self.nav.pop();
                        }
                        self.toast("Recording deleted");
                    }
                    Err(err) => self.error_dialog(err.to_string()),
                }
            }
            DialogKind::DeleteTopic(id) => {
                let result = self.store.borrow_mut().delete_topic(&id);
                match result {
                    Ok(result) => {
                        if self.library_topic.get().as_deref() == Some(id.as_str()) {
                            self.library_topic.set(None);
                        }
                        self.refresh();
                        self.close_dialog();
                        if matches!(self.current_route(), Route::Topic { id: current } if current == id)
                        {
                            self.nav.pop();
                        }
                        self.toast(format!(
                            "Topic deleted · {} recording(s) moved to the parent",
                            result.moved_recordings
                        ));
                    }
                    Err(err) => self.error_dialog(err.to_string()),
                }
            }
            DialogKind::MoveRecording(id) => {
                let topic = self.dialog_topic.get();
                let result = self.store.borrow_mut().move_recording(&id, topic);
                match result {
                    Ok(()) => {
                        self.refresh();
                        self.close_dialog();
                    }
                    Err(err) => self.toast(err.to_string()),
                }
            }
            DialogKind::DefaultTopic => {
                let topic = self
                    .dialog_topic
                    .get()
                    .filter(|id| self.store.borrow().library().topic(id).is_some());
                self.update_settings(|settings| settings.new_recording_topic = topic);
                self.close_dialog();
            }
            DialogKind::Microphone => self.close_dialog(),
            DialogKind::AddEffect => self.close_dialog(),
            DialogKind::EffectParams(_) => self.save_effect_params(),
            DialogKind::Error(_) => self.close_dialog(),
        }
    }

    pub fn start_fx_scan(&self) {
        {
            let mut scan = self.fx_scan.borrow_mut();
            if scan.status == FxScanStatus::Scanning {
                return;
            }
            scan.status = FxScanStatus::Scanning;
            scan.effects.clear();
        }
        if let Ok(mut slot) = self.fx_results.lock() {
            *slot = None;
        }
        let results = self.fx_results.clone();
        let paths = self.plugin_paths();
        std::thread::spawn(move || {
            let scan = aareco_fx::scan_effect_infos(48_000.0, &paths).map_err(|e| e.to_string());
            if let Ok(mut slot) = results.lock() {
                *slot = Some(scan);
            }
            request_frame();
        });
        self.pump_frames();
    }

    pub fn plugin_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let managed = crate::paths::plugins_dir();
        if managed.exists() {
            paths.push(managed);
        }
        for path in self.settings.get().plugin_paths {
            let path = PathBuf::from(path);
            if path.exists() && !paths.contains(&path) {
                paths.push(path);
            }
        }
        paths
    }

    fn pump_frames(&self) {
        std::thread::spawn(|| {
            for _ in 0..240 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                request_frame();
            }
        });
    }

    pub fn poll_fx_scan(&self) {
        if self.fx_scan.borrow().status != FxScanStatus::Scanning {
            return;
        }
        let ready = self.fx_results.lock().ok().and_then(|mut slot| slot.take());
        let Some(result) = ready else {
            return;
        };
        let mut scan = self.fx_scan.borrow_mut();
        match result {
            Ok(effects) => {
                scan.effects = effects;
                scan.status = FxScanStatus::Ready;
            }
            Err(err) => scan.status = FxScanStatus::Failed(err),
        }
    }

    pub fn open_add_effect(&self, recording_id: &str) {
        *self.fx_recording.borrow_mut() = Some(recording_id.to_string());
        self.fx_search.set(String::new());
        if matches!(
            self.fx_scan.borrow().status,
            FxScanStatus::Idle | FxScanStatus::Failed(_)
        ) {
            self.start_fx_scan();
        }
        self.open_dialog(DialogKind::AddEffect, String::new(), None);
    }

    pub fn add_effect(&self, effect: &EffectInfo) {
        let Some(id) = self.fx_recording.borrow().clone() else {
            return;
        };
        let slot = FxSlot {
            backend: effect.backend,
            uri: effect.uri.clone(),
            name: effect.name.clone(),
            bypass: false,
            params: Vec::new(),
        };
        let result = self
            .store
            .borrow_mut()
            .update_recording(&id, |recording| recording.fx.push(slot));
        match result {
            Ok(()) => {
                self.refresh();
                self.close_dialog();
            }
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn remove_effect(&self, recording_id: &str, index: usize) {
        let result = self
            .store
            .borrow_mut()
            .update_recording(recording_id, |recording| {
                if index < recording.fx.len() {
                    recording.fx.remove(index);
                }
            });
        match result {
            Ok(()) => self.refresh(),
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn set_effect_bypass(&self, recording_id: &str, index: usize, bypass: bool) {
        let result = self
            .store
            .borrow_mut()
            .update_recording(recording_id, |recording| {
                if let Some(slot) = recording.fx.get_mut(index) {
                    slot.bypass = bypass;
                }
            });
        match result {
            Ok(()) => self.refresh(),
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn move_effect(&self, recording_id: &str, index: usize, delta: isize) {
        let result = self
            .store
            .borrow_mut()
            .update_recording(recording_id, |recording| {
                let target = index as isize + delta;
                if index < recording.fx.len()
                    && target >= 0
                    && (target as usize) < recording.fx.len()
                {
                    recording.fx.swap(index, target as usize);
                }
            });
        match result {
            Ok(()) => self.refresh(),
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn open_effect_params(&self, recording_id: &str, index: usize) {
        let Some(recording) = self
            .store
            .borrow()
            .library()
            .recording(recording_id)
            .cloned()
        else {
            return;
        };
        let Some(slot) = recording.fx.get(index).cloned() else {
            return;
        };
        match aareco_fx::describe(&slot, recording.sample_rate as f64) {
            Ok(params) => {
                let values = params
                    .iter()
                    .filter(|param| !param.is_hidden)
                    .map(|param| {
                        let key = aareco_fx::param_key(&param.key);
                        let stored = slot
                            .params
                            .iter()
                            .find(|existing| existing.key == key)
                            .map(|existing| existing.value);
                        let range = (param.max - param.min).abs().max(1e-6);
                        let value =
                            stored.unwrap_or(((param.default - param.min) / range).clamp(0.0, 1.0));
                        FxParam { key, value }
                    })
                    .collect();
                *self.fx_params.borrow_mut() = params;
                *self.fx_values.borrow_mut() = values;
                *self.fx_recording.borrow_mut() = Some(recording_id.to_string());
                self.open_dialog(DialogKind::EffectParams(index), slot.name.clone(), None);
            }
            Err(err) => self.error_dialog(format!("Could not read the effect: {err}")),
        }
    }

    pub fn set_effect_param(&self, key: &str, value: f32) {
        if let Some(param) = self
            .fx_values
            .borrow_mut()
            .iter_mut()
            .find(|param| param.key == key)
        {
            param.value = value.clamp(0.0, 1.0);
        }
    }

    pub fn save_effect_params(&self) {
        let DialogKind::EffectParams(index) = self.dialog.get() else {
            return;
        };
        let Some(recording_id) = self.fx_recording.borrow().clone() else {
            return;
        };
        let visible: Vec<String> = self
            .fx_params
            .borrow()
            .iter()
            .filter(|param| !param.is_hidden)
            .map(|param| aareco_fx::param_key(&param.key))
            .collect();
        let values: Vec<FxParam> = self
            .fx_values
            .borrow()
            .iter()
            .filter(|param| visible.contains(&param.key))
            .cloned()
            .collect();
        let result = self
            .store
            .borrow_mut()
            .update_recording(&recording_id, |recording| {
                if let Some(slot) = recording.fx.get_mut(index) {
                    slot.params = values;
                }
            });
        match result {
            Ok(()) => {
                self.refresh();
                self.close_dialog();
            }
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn apply_effects(&self, recording_id: &str) {
        if self.fx_render.borrow().is_some() {
            self.toast("Effects are already being applied");
            return;
        }
        let Some(recording) = self
            .store
            .borrow()
            .library()
            .recording(recording_id)
            .cloned()
        else {
            return;
        };
        if recording.fx.iter().all(|slot| slot.bypass) {
            self.toast("Add an effect first");
            return;
        }
        let source = recording
            .source_file
            .clone()
            .unwrap_or_else(|| recording.file_name.clone());
        let input = self.store.borrow().recordings_dir().join(&source);
        if !input.exists() {
            self.error_dialog("The original audio file is missing.".to_string());
            return;
        }
        let output_name = format!("{}.fx.wav", recording.id);
        let output = self.store.borrow().recordings_dir().join(&output_name);
        if let Some(player) = self.playback.borrow_mut().as_mut() {
            player.stop();
        }
        let job = FxRenderJob {
            recording_id: recording.id.clone(),
            source,
            output_name,
        };
        *self.fx_render.borrow_mut() = Some(job);
        if let Ok(mut slot) = self.fx_render_result.lock() {
            *slot = None;
        }

        let slots = recording.fx.clone();
        let sample_rate = recording.sample_rate as f64;
        let result_slot = self.fx_render_result.clone();
        std::thread::spawn(move || {
            let result = FxChain::new(&slots, sample_rate)
                .and_then(|mut chain| chain.render_wav(&input, &output))
                .map_err(|err| err.to_string());
            if let Ok(mut slot) = result_slot.lock() {
                *slot = Some(result);
            }
            request_frame();
        });
        self.pump_frames();
    }

    pub fn poll_fx_render(&self) {
        let Some(job) = self.fx_render.borrow().as_ref().cloned() else {
            return;
        };
        let ready = self
            .fx_render_result
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        let Some(result) = ready else {
            return;
        };
        *self.fx_render.borrow_mut() = None;
        if let Err(err) = result {
            self.error_dialog(format!("Could not apply the effects: {err}"));
            return;
        }
        let output = self.store.borrow().recordings_dir().join(&job.output_name);
        let peaks = wav::read_peaks(&output, WAVEFORM_POINTS).unwrap_or_default();
        let size = std::fs::metadata(&output)
            .map(|meta| meta.len())
            .unwrap_or(0);
        let result = self
            .store
            .borrow_mut()
            .update_recording(&job.recording_id, |recording| {
                recording.file_name = job.output_name.clone();
                recording.source_file = Some(job.source.clone());
                recording.size_bytes = size;
                recording.waveform = peaks;
            });
        match result {
            Ok(()) => {
                self.refresh();
                self.toast("Effects applied");
            }
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn revert_effects(&self, recording_id: &str) {
        let Some(recording) = self
            .store
            .borrow()
            .library()
            .recording(recording_id)
            .cloned()
        else {
            return;
        };
        let Some(source) = recording.source_file.clone() else {
            return;
        };
        let path = self.store.borrow().recordings_dir().join(&source);
        if !path.exists() {
            self.error_dialog("The original audio file is missing.".to_string());
            return;
        }
        if let Some(player) = self.playback.borrow_mut().as_mut() {
            player.stop();
        }
        let stale = self
            .store
            .borrow()
            .recordings_dir()
            .join(&recording.file_name);
        let peaks = wav::read_peaks(&path, WAVEFORM_POINTS).unwrap_or_default();
        let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        let result = self
            .store
            .borrow_mut()
            .update_recording(recording_id, |recording| {
                recording.file_name = source;
                recording.source_file = None;
                recording.size_bytes = size;
                recording.waveform = peaks;
            });
        match result {
            Ok(()) => {
                if stale != path {
                    let _ = std::fs::remove_file(&stale);
                }
                self.refresh();
                self.toast("Original restored");
            }
            Err(err) => self.toast(err.to_string()),
        }
    }

    pub fn remove_plugin_path(&self, path: &str) {
        self.update_settings(|settings| settings.plugin_paths.retain(|kept| kept != path));
        self.start_fx_scan();
    }

    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    pub fn add_plugin_path(&self) {
        let Some(directory) = rlobkit_dialogs::blocking_pick_directory("Choose a plugin folder")
        else {
            return;
        };
        let path = directory.to_string_lossy().to_string();
        if self.settings.get().plugin_paths.contains(&path) {
            return;
        }
        self.update_settings(|settings| settings.plugin_paths.push(path));
        self.start_fx_scan();
    }

    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    pub fn import_plugin_bundle(&self) {
        let files = rlobkit_dialogs::blocking_pick_files("Import a CLAP plugin", &["clap"]);
        if files.is_empty() {
            return;
        }
        let directory = crate::paths::plugins_dir();
        if let Err(err) = std::fs::create_dir_all(&directory) {
            self.error_dialog(format!("Could not create the plugin folder: {err}"));
            return;
        }
        let mut imported = 0usize;
        for file in files {
            let Some(name) = file.file_name() else {
                continue;
            };
            if std::fs::copy(&file, directory.join(name)).is_ok() {
                imported += 1;
            }
        }
        if imported == 0 {
            self.error_dialog("Could not copy the plugin into the library".to_string());
            return;
        }
        self.start_fx_scan();
        self.toast(format!("Imported {imported} plugin(s)"));
    }

    pub fn can_go_back(&self) -> bool {
        self.dialog_state.is_visible()
            || self.current_route() == Route::Record
            || !self.current_route().is_root()
    }

    pub fn go_back(&self) -> bool {
        if self.dialog_state.is_visible() {
            self.on_dialog_dismiss();
            return true;
        }
        if self.current_route() == Route::Record {
            self.open_discard_dialog();
            return true;
        }
        self.nav.pop()
    }

    pub fn backup_now(&self) {
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            let Some(directory) =
                rlobkit_dialogs::blocking_pick_directory("Choose a backup folder")
            else {
                return;
            };
            match self.store.borrow().backup_to(&directory) {
                Ok(path) => {
                    self.update_settings(|settings| settings.last_backup_at = Some(now_ms()));
                    self.toast(format!("Backup written to {}", path.display()));
                }
                Err(err) => self.error_dialog(format!("Backup failed: {err}")),
            }
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        {
            self.toast("Backup uses a folder picker on desktop for now");
        }
    }

    pub fn restore_backup(&self) {
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            let Some(directory) =
                rlobkit_dialogs::blocking_pick_directory("Choose an Aareco backup folder")
            else {
                return;
            };
            let result = self.store.borrow_mut().import_backup(&directory);
            match result {
                Ok(result) => {
                    self.refresh();
                    self.toast(format!(
                        "Restored {} recording(s) and {} topic(s) · {} skipped",
                        result.recordings, result.topics, result.skipped
                    ));
                }
                Err(err) => self.error_dialog(format!("Restore failed: {err}")),
            }
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        {
            self.toast("Restore uses a folder picker on desktop for now");
        }
    }

    pub fn reveal_data_folder(&self) {
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            match open_in_file_manager(&self.root) {
                Ok(()) => {}
                Err(err) => self.error_dialog(format!("Could not open the data folder: {err}")),
            }
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        {
            self.toast(format!("Library lives in {}", self.root.display()));
        }
    }
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn open_in_file_manager(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "windows")]
    let opener = "explorer";

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "revealing folders is not supported on this platform",
    ));

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    std::process::Command::new(opener)
        .arg(path)
        .spawn()
        .map(|_| ())
}

fn adopt_existing_files(store: &mut Store) {
    let dir = store.recordings_dir();
    let mut known: HashSet<String> = store
        .library()
        .recordings
        .iter()
        .flat_map(|recording| {
            std::iter::once(recording.file_name.clone()).chain(recording.source_file.clone())
        })
        .collect();

    for recovered in recover_partial_files(&dir) {
        let Some(file_name) = recovered
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if known.contains(&file_name) {
            continue;
        }
        let created_at = crate::ui::common::file_date(&recovered.path, now_ms());
        let id = unique_id(store, &recovered.path);
        let recording = Recording {
            id,
            title: format!(
                "Recovered · {}",
                crate::ui::common::timestamp_date(created_at)
            ),
            file_name: file_name.clone(),
            topic_id: None,
            created_at,
            duration_ms: recovered.duration_ms,
            sample_rate: recovered.sample_rate,
            size_bytes: std::fs::metadata(&recovered.path)
                .map(|m| m.len())
                .unwrap_or(0),
            marks: Vec::new(),
            waveform: recovered.peaks,
            source_file: None,
            fx: Vec::new(),
        };
        if store.add_recording(recording).is_ok() {
            known.insert(file_name);
        }
    }

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("wav") {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".fx.wav"))
        {
            continue;
        }
        let Some(file_name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if known.contains(&file_name) {
            continue;
        }
        let Ok(info) = wav::inspect_wav(&path) else {
            continue;
        };
        let created_at = crate::ui::common::file_date(&path, now_ms());
        let id = unique_id(store, &path);
        let recording = Recording {
            id,
            title: format!(
                "Recovered · {}",
                crate::ui::common::timestamp_date(created_at)
            ),
            file_name: file_name.clone(),
            topic_id: None,
            created_at,
            duration_ms: info.duration_ms(),
            sample_rate: info.sample_rate,
            size_bytes: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            marks: Vec::new(),
            waveform: wav::read_peaks(&path, WAVEFORM_POINTS).unwrap_or_default(),
            source_file: None,
            fx: Vec::new(),
        };
        if store.add_recording(recording).is_ok() {
            known.insert(file_name);
        }
    }
}

fn unique_id(store: &Store, path: &std::path::Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    if store.library().recording(&stem).is_some() {
        Uuid::new_v4().to_string()
    } else {
        stem
    }
}
