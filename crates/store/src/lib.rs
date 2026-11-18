use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;
use uuid::Uuid;

pub mod model;

pub use model::{LIBRARY_VERSION, Library, Mark, Recording, Topic, WAVEFORM_POINTS};

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
    #[error("topic name cannot be empty")]
    EmptyName,
    #[error("a topic cannot be moved inside itself")]
    Cycle,
    #[error("backup failed: {0}")]
    Backup(String),
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
    pub recordings: usize,
    pub skipped: usize,
}

impl Store {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(root.join("recordings")).map_err(|e| io_error(&root, e))?;
        let path = root.join("library.json");
        let library = if path.exists() {
            let bytes = std::fs::read(&path).map_err(|e| io_error(&path, e))?;
            match serde_json::from_slice::<Library>(&bytes) {
                Ok(library) => library,
                Err(source) => {
                    let stamp = Local::now().format("%Y%m%d-%H%M%S");
                    let broken = root.join(format!("library.json.broken-{stamp}"));
                    let _ = std::fs::rename(&path, &broken);
                    return Err(StoreError::Json { path, source });
                }
            }
        } else {
            Library::new()
        };
        let store = Self { root, library };
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
        let path = self.library_path();
        let tmp = self.root.join("library.json.tmp");
        let json = serde_json::to_vec_pretty(&self.library).map_err(|source| StoreError::Json {
            path: path.clone(),
            source,
        })?;
        {
            let mut file = File::create(&tmp).map_err(|e| io_error(&tmp, e))?;
            file.write_all(&json).map_err(|e| io_error(&tmp, e))?;
            file.sync_all().map_err(|e| io_error(&tmp, e))?;
        }
        if let Err(e) = std::fs::rename(&tmp, &path) {
            if path.exists() {
                let _ = std::fs::remove_file(&path);
                std::fs::rename(&tmp, &path).map_err(|e| io_error(&path, e))?;
            } else {
                return Err(io_error(&path, e));
            }
        }
        Ok(())
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
            if parent == id || self.library.descendants(id).iter().any(|child| child == parent) {
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
        let path = self.recording_path(&self.library.recordings[index]);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| io_error(&path, e))?;
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
            let source = self.recording_path(recording);
            if source.exists() {
                let target = dest.join("recordings").join(&recording.file_name);
                std::fs::copy(&source, &target).map_err(|e| io_error(&source, e))?;
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
        let backup: Library = serde_json::from_slice(&bytes).map_err(|source| StoreError::Json {
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
            std::fs::copy(&source_file, &target_file)
                .map_err(|e| io_error(&source_file, e))?;
            let mut restored = recording.clone();
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
        }
    }

    #[test]
    fn persists_topics_and_recordings() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut store = Store::open(dir.path()).unwrap();
            let topic = store.add_topic("Work", None).unwrap();
            store.add_recording(recording("one", Some(&topic.id))).unwrap();
        }
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.library().topics.len(), 1);
        assert_eq!(store.library().recordings.len(), 1);
        assert_eq!(store.stats().bytes, 96_000);
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
