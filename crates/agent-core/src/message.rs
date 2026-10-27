use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Content, ContentBody, Usage, content::Properties};

/// Open-ended role; vendor-defined roles survive serialization.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Role(pub String);

impl Role {
    pub fn user() -> Self {
        Self("user".into())
    }
    pub fn assistant() -> Self {
        Self("assistant".into())
    }
    pub fn system() -> Self {
        Self("system".into())
    }
    pub fn tool() -> Self {
        Self("tool".into())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Source {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Type")]
    pub kind: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Message {
    #[serde(skip)]
    pub raw: crate::raw::RawRepresentations,
    #[serde(rename = "ID")]
    pub id: String,
    pub role: Role,
    pub author_name: String,
    pub contents: Vec<Content>,
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    pub additional_properties: Properties,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::user(),
            contents: vec![Content::text(text)],
            ..Self::default()
        }
    }

    pub fn text(&self) -> String {
        self.contents.iter().filter_map(Content::as_text).collect()
    }

    pub fn usage(&self) -> Usage {
        let mut usage = Usage::default();
        for content in &self.contents {
            if let ContentBody::Usage { details } = &content.body {
                usage.add(details);
            }
        }
        usage
    }

    /// Rust values are deeply cloned, so later edits cannot mutate the source.
    pub fn with_source(&self, source: Source) -> Self {
        Self {
            source,
            ..self.clone()
        }
    }
}
