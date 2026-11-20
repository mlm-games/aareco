use serde::{Deserialize, Serialize};

pub const LIBRARY_VERSION: u32 = 1;
pub const WAVEFORM_POINTS: usize = 600;

fn default_version() -> u32 {
    LIBRARY_VERSION
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Library {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub topics: Vec<Topic>,
    #[serde(default)]
    pub recordings: Vec<Recording>,
    #[serde(default)]
    pub templates: Vec<RecordingTemplate>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Topic {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Recording {
    pub id: String,
    pub title: String,
    pub file_name: String,
    pub topic_id: Option<String>,
    pub created_at: i64,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub size_bytes: u64,
    #[serde(default)]
    pub marks: Vec<Mark>,
    #[serde(default)]
    pub waveform: Vec<u8>,
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub fx: Vec<FxSlot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecordingTemplate {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub fx: Vec<FxSlot>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum FxBackend {
    #[default]
    Vst3,
    Builtin,
    Lv2,
    Clap,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FxSlot {
    #[serde(default)]
    pub backend: FxBackend,
    pub uri: String,
    pub name: String,
    #[serde(default)]
    pub bypass: bool,
    #[serde(default)]
    pub params: Vec<FxParam>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FxParam {
    pub key: String,
    pub value: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Mark {
    pub id: String,
    pub offset_ms: u64,
    #[serde(default)]
    pub label: Option<String>,
}

impl Library {
    pub fn new() -> Self {
        Self {
            version: LIBRARY_VERSION,
            topics: Vec::new(),
            recordings: Vec::new(),
            templates: Vec::new(),
        }
    }

    pub fn topic(&self, id: &str) -> Option<&Topic> {
        self.topics.iter().find(|topic| topic.id == id)
    }

    pub fn recording(&self, id: &str) -> Option<&Recording> {
        self.recordings.iter().find(|recording| recording.id == id)
    }

    pub fn template(&self, id: &str) -> Option<&RecordingTemplate> {
        self.templates.iter().find(|template| template.id == id)
    }

    pub fn children_of(&self, parent: Option<&str>) -> Vec<&Topic> {
        let mut children: Vec<&Topic> = self
            .topics
            .iter()
            .filter(|topic| topic.parent_id.as_deref() == parent)
            .collect();
        children.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.created_at.cmp(&b.created_at))
        });
        children
    }

    pub fn descendants(&self, root: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_string()];
        while let Some(id) = stack.pop() {
            for topic in &self.topics {
                if topic.parent_id.as_deref() == Some(id.as_str()) {
                    out.push(topic.id.clone());
                    stack.push(topic.id.clone());
                }
            }
        }
        out
    }

    pub fn breadcrumb(&self, id: &str) -> String {
        let mut parts = Vec::new();
        let mut current = self.topic(id);
        let mut guard = 0;
        while let Some(topic) = current {
            parts.push(topic.name.clone());
            guard += 1;
            if guard > 64 {
                break;
            }
            current = topic
                .parent_id
                .as_deref()
                .and_then(|parent| self.topic(parent));
        }
        parts.reverse();
        parts.join(" / ")
    }

    pub fn recording_count(&self, topic: &str, include_children: bool) -> usize {
        let mut ids = vec![topic.to_string()];
        if include_children {
            ids.extend(self.descendants(topic));
        }
        self.recordings
            .iter()
            .filter(|recording| {
                recording
                    .topic_id
                    .as_deref()
                    .is_some_and(|id| ids.iter().any(|known| known == id))
            })
            .count()
    }
}
