use std::path::{Path, PathBuf};

use chrono::Local;
use game_utils::save_store::{LoadStatus, SaveStore};
use game_utils::storage::FsStorage;
use uuid::Uuid;

pub mod model;

pub use model::{
    FxBackend, FxParam, FxSlot, LIBRARY_VERSION, Library, Mark, Recording, RecordingTemplate,
    Topic, WAVEFORM_POINTS,
};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("file error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("library file at {path} is not valid: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("topic {0:?} does not exist")]
    TopicNotFound(String),
    #[error("recording {0:?} does not exist")]
    RecordingNotFound(String),
    #[error("template {0:?} does not exist")]
    TemplateNotFound(String),
    #[error("topic name cannot be empty")]
    EmptyName,
    #[error("a topic cannot be moved inside itself")]
    Cycle,
    #[error("backup failed: {0}")]
    Backup(String),
    #[error("could not write the library: {0}")]
    Save(String),
    #[error("library file at {path} is damaged and was set aside")]
    Corrupt { path: PathBuf },
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

fn io_error(path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub struct Store {
    root: PathBuf,
    library: Library,
    file: LibraryFile,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StorageStats {
    pub recordings: usize,
    pub topics: usize,
    pub bytes: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeleteTopicResult {
    pub deleted_topics: usize,
    pub moved_recordings: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RestoreResult {
    pub topics: usize,
    pub templates: usize,
    pub recordings: usize,
    pub skipped: usize,
}

type LibraryFile = SaveStore<FsStorage>;

impl Store {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(root.join("recordings")).map_err(|e| io_error(&root, e))?;
        let path = root.join("library.json");
        let file = LibraryFile::new(&root, "library.json");
        let loaded = file.load(&LibraryFile::is_intact_json, &[]);
        let library = match loaded.data {
            Some(bytes) => {
                serde_json::from_slice::<Library>(&bytes).map_err(|source| StoreError::Json {
                    path: loaded
                        .recovered_from
                        .clone()
                        .unwrap_or_else(|| path.clone()),
                    source,
                })?
            }
            None if loaded.status == LoadStatus::Corrupt
                || loaded.status == LoadStatus::Unreadable =>
            {
                return Err(StoreError::Corrupt { path });
            }
            None => Library::new(),
        };
        let store = Self {
            root,
            library,
            file,
        };
        if !store.library_path().exists() {
            store.save()?;
        }
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn library_path(&self) -> PathBuf {
        self.root.join("library.json")
    }

    pub fn recordings_dir(&self) -> PathBuf {
        self.root.join("recordings")
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn recording_path(&self, recording: &Recording) -> PathBuf {
        self.recordings_dir().join(&recording.file_name)
    }

    pub fn recording_file_exists(&self, recording: &Recording) -> bool {
        self.recording_path(recording).exists()
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_vec_pretty(&self.library).map_err(|source| StoreError::Json {
            path: self.library_path(),
            source,
        })?;
        self.file
            .write(&json)
            .map_err(|e| StoreError::Save(e.to_string()))
    }

    pub fn add_topic(&mut self, name: &str, parent_id: Option<String>) -> Result<Topic> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::EmptyName);
        }
        if let Some(parent) = &parent_id
            && self.library.topic(parent).is_none()
        {
            return Err(StoreError::TopicNotFound(parent.clone()));
        }
        let topic = Topic {
            id: Uuid::new_v4().to_string(),
            parent_id,
            name: name.to_string(),
            created_at: now_ms(),
        };
        self.library.topics.push(topic.clone());
        self.save()?;
        Ok(topic)
    }

    pub fn rename_topic(&mut self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::EmptyName);
        }
        let topic = self
            .library
            .topics
            .iter_mut()
            .find(|topic| topic.id == id)
            .ok_or_else(|| StoreError::TopicNotFound(id.to_string()))?;
        topic.name = name.to_string();
        self.save()
    }

    pub fn move_topic(&mut self, id: &str, parent_id: Option<String>) -> Result<()> {
        if let Some(parent) = &parent_id {
            if self.library.topic(parent).is_none() {
                return Err(StoreError::TopicNotFound(parent.clone()));
            }
            if parent == id
                || self
                    .library
                    .descendants(id)
                    .iter()
                    .any(|child| child == parent)
            {
                return Err(StoreError::Cycle);
            }
        }
        let topic = self
            .library
            .topics
            .iter_mut()
            .find(|topic| topic.id == id)
            .ok_or_else(|| StoreError::TopicNotFound(id.to_string()))?;
        topic.parent_id = parent_id;
        self.save()
    }

    /// Removes a topic and its descendants. Recordings that lived in the
    /// removed topics move to the deleted topic's parent (Inbox at the root).
    pub fn delete_topic(&mut self, id: &str) -> Result<DeleteTopicResult> {
        let topic = self
            .library
            .topic(id)
            .ok_or_else(|| StoreError::TopicNotFound(id.to_string()))?
            .clone();
        let mut removed = vec![id.to_string()];
        removed.extend(self.library.descendants(id));

        let mut moved = 0;
        for recording in &mut self.library.recordings {
            if recording
                .topic_id
                .as_deref()
                .is_some_and(|topic_id| removed.iter().any(|id| id == topic_id))
            {
                recording.topic_id = topic.parent_id.clone();
                moved += 1;
            }
        }
        self.library
            .topics
            .retain(|topic| !removed.iter().any(|id| id == &topic.id));
        self.save()?;
        Ok(DeleteTopicResult {
            deleted_topics: removed.len(),
            moved_recordings: moved,
        })
    }

    pub fn add_template(&mut self, name: &str, fx: Vec<FxSlot>) -> Result<RecordingTemplate> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::EmptyName);
        }
        let now = now_ms();
        let template = RecordingTemplate {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            created_at: now,
            updated_at: now,
            fx,
        };
        self.library.templates.push(template.clone());
        self.save()?;
        Ok(template)
    }

    pub fn rename_template(&mut self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::EmptyName);
        }
        let template = self
            .library
            .templates
            .iter_mut()
            .find(|template| template.id == id)
            .ok_or_else(|| StoreError::TemplateNotFound(id.to_string()))?;
        template.name = name.to_string();
        template.updated_at = now_ms();
        self.save()
    }

    pub fn update_template(&mut self, id: &str, fx: Vec<FxSlot>) -> Result<()> {
        let template = self
            .library
            .templates
            .iter_mut()
            .find(|template| template.id == id)
            .ok_or_else(|| StoreError::TemplateNotFound(id.to_string()))?;
        template.fx = fx;
        template.updated_at = now_ms();
        self.save()
    }

    pub fn duplicate_template(&mut self, id: &str) -> Result<RecordingTemplate> {
        let source = self
            .library
            .template(id)
            .ok_or_else(|| StoreError::TemplateNotFound(id.to_string()))?
            .clone();
        self.add_template(&format!("{} copy", source.name), source.fx)
    }

    pub fn delete_template(&mut self, id: &str) -> Result<()> {
        let index = self
            .library
            .templates
            .iter()
            .position(|template| template.id == id)
            .ok_or_else(|| StoreError::TemplateNotFound(id.to_string()))?;
        self.library.templates.remove(index);
        self.save()
    }

    pub fn add_recording(&mut self, recording: Recording) -> Result<()> {
        if self.library.recording(&recording.id).is_some() {
            self.library
                .recordings
                .retain(|existing| existing.id != recording.id);
        }
        self.library.recordings.push(recording);
        self.save()
    }

    pub fn update_recording(
        &mut self,
        id: &str,
        update: impl FnOnce(&mut Recording),
    ) -> Result<()> {
        let recording = self
            .library
            .recordings
            .iter_mut()
            .find(|recording| recording.id == id)
            .ok_or_else(|| StoreError::RecordingNotFound(id.to_string()))?;
        update(recording);
        self.save()
    }

    pub fn rename_recording(&mut self, id: &str, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(StoreError::EmptyName);
        }
        self.update_recording(id, |recording| recording.title = title.to_string())
    }

    pub fn move_recording(&mut self, id: &str, topic_id: Option<String>) -> Result<()> {
        if let Some(topic) = &topic_id
            && self.library.topic(topic).is_none()
        {
            return Err(StoreError::TopicNotFound(topic.clone()));
        }
        self.update_recording(id, |recording| recording.topic_id = topic_id.clone())
    }

    pub fn add_mark(&mut self, id: &str, offset_ms: u64, label: Option<String>) -> Result<Mark> {
        let mark = Mark {
            id: Uuid::new_v4().to_string(),
            offset_ms,
            label,
        };
        let mark_for_insert = mark.clone();
        self.update_recording(id, |recording| {
            recording.marks.push(mark_for_insert);
            recording.marks.sort_by_key(|mark| mark.offset_ms);
        })?;
        Ok(mark)
    }

    pub fn delete_mark(&mut self, recording_id: &str, mark_id: &str) -> Result<()> {
        self.update_recording(recording_id, |recording| {
            recording.marks.retain(|mark| mark.id != mark_id);
        })
    }

    pub fn nudge_mark(&mut self, recording_id: &str, mark_id: &str, delta_ms: i64) -> Result<()> {
        self.update_recording(recording_id, |recording| {
            let duration = recording.duration_ms as i64;
            if let Some(mark) = recording.marks.iter_mut().find(|mark| mark.id == mark_id) {
                mark.offset_ms = (mark.offset_ms as i64 + delta_ms).clamp(0, duration) as u64;
            }
            recording.marks.sort_by_key(|mark| mark.offset_ms);
        })
    }

    pub fn delete_recording(&mut self, id: &str) -> Result<()> {
        let index = self
            .library
            .recordings
            .iter()
            .position(|recording| recording.id == id)
            .ok_or_else(|| StoreError::RecordingNotFound(id.to_string()))?;
        let recording = &self.library.recordings[index];
        let mut files = vec![self.recording_path(recording)];
        if let Some(source) = &recording.source_file {
            files.push(self.recordings_dir().join(source));
        }
        for path in files {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| io_error(&path, e))?;
            }
        }
        self.library.recordings.remove(index);
        self.save()
    }

    pub fn recordings_sorted(&self) -> Vec<&Recording> {
        let mut recordings: Vec<&Recording> = self.library.recordings.iter().collect();
        recordings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        recordings
    }

    pub fn recordings_in(&self, topic_id: Option<&str>, include_children: bool) -> Vec<&Recording> {
        let ids = topic_id.map(|id| {
            let mut ids = vec![id.to_string()];
            if include_children {
                ids.extend(self.library.descendants(id));
            }
            ids
        });
        let mut recordings: Vec<&Recording> = self
            .library
            .recordings
            .iter()
            .filter(|recording| match (&ids, &recording.topic_id) {
                (None, None) => true,
                (Some(ids), Some(topic)) => ids.iter().any(|id| id == topic),
                _ => false,
            })
            .collect();
        recordings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        recordings
    }

    pub fn search(&self, query: &str, topic_id: Option<&str>) -> Vec<&Recording> {
        let query = query.trim().to_lowercase();
        let topic_ids = topic_id.map(|id| {
            let mut ids = vec![id.to_string()];
            ids.extend(self.library.descendants(id));
            ids
        });
        let mut recordings: Vec<&Recording> = self
            .library
            .recordings
            .iter()
            .filter(|recording| {
                if let Some(ids) = &topic_ids {
                    let Some(topic) = &recording.topic_id else {
                        return false;
                    };
                    if !ids.iter().any(|id| id == topic) {
                        return false;
                    }
                }
                if query.is_empty() {
                    return true;
                }
                recording.title.to_lowercase().contains(&query)
                    || recording
                        .topic_id
                        .as_deref()
                        .map(|id| self.library.breadcrumb(id).to_lowercase().contains(&query))
                        .unwrap_or(false)
                    || recording.marks.iter().any(|mark| {
                        mark.label
                            .as_deref()
                            .is_some_and(|label| label.to_lowercase().contains(&query))
                    })
            })
            .collect();
        recordings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        recordings
    }

    pub fn stats(&self) -> StorageStats {
        StorageStats {
            recordings: self.library.recordings.len(),
            topics: self.library.topics.len(),
            bytes: self
                .library
                .recordings
                .iter()
                .map(|recording| recording.size_bytes)
                .sum(),
            duration_ms: self
                .library
                .recordings
                .iter()
                .map(|recording| recording.duration_ms)
                .sum(),
        }
    }

    pub fn backup_to(&self, parent: &Path) -> Result<PathBuf> {
        self.save()?;
        std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let dest = parent.join(format!("aareco-backup-{stamp}"));
        std::fs::create_dir_all(dest.join("recordings")).map_err(|e| io_error(&dest, e))?;
        std::fs::copy(self.library_path(), dest.join("library.json"))
            .map_err(|e| io_error(&dest, e))?;

        let mut copied = 0usize;
        for recording in &self.library.recordings {
            let mut names = vec![recording.file_name.clone()];
            if let Some(source) = recording
                .source_file
                .as_ref()
                .filter(|source| is_safe_file_name(source))
                && *source != recording.file_name
            {
                names.push(source.clone());
            }
            let mut copied_recording = false;
            for name in names {
                let source = self.recordings_dir().join(&name);
                if !source.exists() {
                    continue;
                }
                let target = dest.join("recordings").join(&name);
                std::fs::copy(&source, &target).map_err(|e| io_error(&source, e))?;
                copied_recording = true;
            }
            if copied_recording {
                copied += 1;
            }
        }

        let manifest = serde_json::json!({
            "app": "aareco",
            "version": LIBRARY_VERSION,
            "created_at": now_ms(),
            "recordings": copied,
        });
        let manifest_path = dest.join("backup.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).map_err(|e| StoreError::Backup(e.to_string()))?,
        )
        .map_err(|e| io_error(&manifest_path, e))?;
        Ok(dest)
    }

    pub fn import_backup(&mut self, source: &Path) -> Result<RestoreResult> {
        let library_path = source.join("library.json");
        let bytes = std::fs::read(&library_path).map_err(|e| io_error(&library_path, e))?;
        let backup: Library =
            serde_json::from_slice(&bytes).map_err(|source| StoreError::Json {
                path: library_path,
                source,
            })?;

        let mut added_topics = 0usize;
        for topic in &backup.topics {
            if self.library.topic(&topic.id).is_none() {
                self.library.topics.push(topic.clone());
                added_topics += 1;
            }
        }
        let known_topics: std::collections::HashSet<String> = self
            .library
            .topics
            .iter()
            .map(|topic| topic.id.clone())
            .collect();
        for topic in &mut self.library.topics {
            if topic
                .parent_id
                .as_deref()
                .is_some_and(|parent| !known_topics.contains(parent))
            {
                topic.parent_id = None;
            }
        }

        let mut added_templates = 0usize;
        for template in &backup.templates {
            if self.library.template(&template.id).is_none() {
                self.library.templates.push(template.clone());
                added_templates += 1;
            }
        }

        let source_dir = source.join("recordings");
        let mut added_recordings = 0usize;
        let mut skipped = 0usize;
        for recording in &backup.recordings {
            if self.library.recording(&recording.id).is_some()
                || !is_safe_file_name(&recording.file_name)
            {
                skipped += 1;
                continue;
            }
            let source_file = source_dir.join(&recording.file_name);
            let target_file = self.recordings_dir().join(&recording.file_name);
            if !source_file.is_file() || target_file.exists() {
                skipped += 1;
                continue;
            }
            std::fs::copy(&source_file, &target_file).map_err(|e| io_error(&source_file, e))?;
            let mut restored = recording.clone();
            if let Some(source) = restored
                .source_file
                .clone()
                .filter(|source| is_safe_file_name(source))
            {
                let backup_source = source_dir.join(&source);
                let target_source = self.recordings_dir().join(&source);
                if backup_source.is_file() && !target_source.exists() {
                    std::fs::copy(&backup_source, &target_source)
                        .map_err(|e| io_error(&backup_source, e))?;
                }
            } else {
                restored.source_file = None;
            }
            if restored
                .topic_id
                .as_deref()
                .is_some_and(|id| self.library.topic(id).is_none())
            {
                restored.topic_id = None;
            }
            self.library.recordings.push(restored);
            added_recordings += 1;
        }

        self.save()?;
        Ok(RestoreResult {
            topics: added_topics,
            templates: added_templates,
            recordings: added_recordings,
            skipped,
        })
    }
}

fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.contains('\\')
        && name.ends_with(".wav")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(id: &str, topic: Option<&str>) -> Recording {
        Recording {
            id: id.to_string(),
            title: id.to_string(),
            file_name: format!("{id}.wav"),
            topic_id: topic.map(str::to_string),
            created_at: now_ms(),
            duration_ms: 1000,
            sample_rate: 48_000,
            size_bytes: 96_000,
            marks: Vec::new(),
            waveform: vec![0; 10],
            source_file: None,
            fx: Vec::new(),
        }
    }

    #[test]
    fn persists_topics_and_recordings() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut store = Store::open(dir.path()).unwrap();
            let topic = store.add_topic("Work", None).unwrap();
            store
                .add_recording(recording("one", Some(&topic.id)))
                .unwrap();
        }
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.library().topics.len(), 1);
        assert_eq!(store.library().recordings.len(), 1);
        assert_eq!(store.stats().bytes, 96_000);
    }

    #[test]
    fn backups_keep_processed_and_original_audio() {
        let dir = tempfile::tempdir().unwrap();
        let backup_parent = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let mut processed = recording("effect", None);
        processed.file_name = "effect.fx.wav".to_string();
        processed.source_file = Some("effect.wav".to_string());
        store.add_recording(processed).unwrap();
        let template = store.add_template("Doc voice", Vec::new()).unwrap();
        for name in ["effect.fx.wav", "effect.wav"] {
            std::fs::write(store.recordings_dir().join(name), b"wav").unwrap();
        }

        let backup = store.backup_to(backup_parent.path()).unwrap();
        assert!(backup.join("recordings/effect.fx.wav").is_file());
        assert!(backup.join("recordings/effect.wav").is_file());
        let backup_library: Library =
            serde_json::from_slice(&std::fs::read(backup.join("library.json")).unwrap()).unwrap();
        assert_eq!(backup_library.templates.len(), 1);

        let restored_dir = tempfile::tempdir().unwrap();
        let mut restored = Store::open(restored_dir.path()).unwrap();
        let result = restored.import_backup(&backup).unwrap();
        assert_eq!(result.recordings, 1);
        assert_eq!(result.templates, 1);
        assert!(restored.library().template(&template.id).is_some());
        assert!(restored.recordings_dir().join("effect.fx.wav").is_file());
        assert!(restored.recordings_dir().join("effect.wav").is_file());
        assert_eq!(
            restored.library().recording("effect").unwrap().source_file,
            Some("effect.wav".to_string())
        );
    }

    #[test]
    fn templates_persist_and_delete_independently_of_recordings() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let slot = FxSlot {
            backend: FxBackend::Vst3,
            uri: "/plugins/eq.vst3".to_string(),
            name: "EQ".to_string(),
            bypass: false,
            params: Vec::new(),
        };
        let template = store.add_template("Voice", vec![slot]).unwrap();
        let template_id = template.id.clone();
        let mut owner = recording("owned", None);
        owner.fx = template.fx.clone();
        store.add_recording(owner).unwrap();

        drop(store);
        let mut store = Store::open(dir.path()).unwrap();
        assert_eq!(store.library().template(&template_id).unwrap().fx.len(), 1);
        store.delete_template(&template_id).unwrap();
        assert_eq!(store.library().recording("owned").unwrap().fx.len(), 1);
        assert!(store.library().template(&template_id).is_none());
    }

    #[test]
    fn deleting_a_topic_reparents_recordings_and_removes_children() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let parent = store.add_topic("Parent", None).unwrap();
        let child = store.add_topic("Child", Some(parent.id.clone())).unwrap();
        store
            .add_recording(recording("parent-rec", Some(&parent.id)))
            .unwrap();
        store
            .add_recording(recording("child-rec", Some(&child.id)))
            .unwrap();

        let result = store.delete_topic(&child.id).unwrap();
        assert_eq!(result.deleted_topics, 1);
        assert_eq!(result.moved_recordings, 1);
        assert_eq!(
            store.library().recording("child-rec").unwrap().topic_id,
            Some(parent.id.clone())
        );

        let result = store.delete_topic(&parent.id).unwrap();
        assert_eq!(result.deleted_topics, 1);
        assert_eq!(result.moved_recordings, 2);
        assert!(store.library().topics.is_empty());
        assert_eq!(
            store.library().recording("parent-rec").unwrap().topic_id,
            None
        );
    }
}
