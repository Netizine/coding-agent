// Adapted from Microsoft Agent Framework Go agent/response.go (MIT).
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Content, Message, Role, Usage, content::Properties, raw::RawRepresentations};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ResponseUpdate {
    #[serde(skip)]
    pub raw: RawRepresentations,
    #[serde(rename = "AgentID")]
    pub agent_id: String,
    #[serde(rename = "MessageID")]
    pub message_id: String,
    #[serde(rename = "ResponseID")]
    pub response_id: String,
    #[serde(rename = "ConversationID")]
    pub conversation_id: Option<String>,
    pub finish_reason: String,
    pub author_name: String,
    pub role: Role,
    pub continuation_token: String,
    pub created_at: Option<DateTime<Utc>>,
    pub contents: Vec<Content>,
    pub additional_properties: Properties,
}

impl ResponseUpdate {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            contents: vec![Content::text(text)],
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Response {
    #[serde(skip)]
    pub raw: RawRepresentations,
    #[serde(rename = "AgentID")]
    pub agent_id: String,
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "ConversationID")]
    pub conversation_id: Option<String>,
    pub finish_reason: String,
    pub continuation_token: String,
    pub created_at: Option<DateTime<Utc>>,
    pub messages: Vec<Message>,
    pub additional_properties: Properties,
}

fn different(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && a != b
}
fn replace_if_present(target: &mut String, value: String) {
    if !value.is_empty() {
        *target = value;
    }
}
fn valid(time: Option<DateTime<Utc>>) -> bool {
    time.is_some_and(|t| t > DateTime::UNIX_EPOCH)
}

impl Response {
    /// Fold into the last compatible message, preserving Go ordering semantics.
    pub fn update(&mut self, update: ResponseUpdate) {
        let needs_message = self.messages.last().is_none_or(|message| {
            different(&message.id, &update.message_id)
                || different(&message.author_name, &update.author_name)
                || different(&message.role.0, &update.role.0)
        });
        if needs_message {
            self.messages.push(Message {
                role: Role::assistant(),
                ..Message::default()
            });
        }
        let message = self.messages.last_mut().expect("message inserted");
        replace_if_present(&mut message.author_name, update.author_name);
        replace_if_present(&mut message.role.0, update.role.0);
        if !valid(message.created_at) && valid(update.created_at) {
            message.created_at = update.created_at;
        }
        message.contents.extend(update.contents);
        message.raw.append(&update.raw);
        self.raw.append(&update.raw);
        if update.message_id.is_empty() {
            self.additional_properties
                .extend(update.additional_properties);
        } else {
            message
                .additional_properties
                .extend(update.additional_properties);
        }
        replace_if_present(&mut message.id, update.message_id);
        replace_if_present(&mut self.agent_id, update.agent_id);
        replace_if_present(&mut self.id, update.response_id);
        replace_if_present(&mut self.finish_reason, update.finish_reason);
        if update.conversation_id.is_some() {
            self.conversation_id = update.conversation_id;
        }
        self.continuation_token = update.continuation_token;
        if !valid(self.created_at) && valid(update.created_at) {
            self.created_at = update.created_at;
        }
    }

    pub fn text(&self) -> String {
        self.messages
            .iter()
            .map(Message::text)
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn usage(&self) -> Usage {
        let mut total = Usage::default();
        for message in &self.messages {
            total.add(&message.usage());
        }
        total
    }

    /// Combine content using the Go baseline's content-specific merge rules.
    pub fn coalesce(&mut self) {
        for message in &mut self.messages {
            message.contents = crate::coalesce::coalesce(std::mem::take(&mut message.contents));
        }
    }

    /// Project messages back into updates; metadata-only response fields follow the Go policy.
    pub fn to_updates(&self) -> Vec<ResponseUpdate> {
        let mut updates: Vec<_> = self
            .messages
            .iter()
            .map(|m| ResponseUpdate {
                raw: m.raw.clone(),
                additional_properties: m.additional_properties.clone(),
                agent_id: self.agent_id.clone(),
                message_id: m.id.clone(),
                response_id: self.id.clone(),
                conversation_id: self.conversation_id.clone(),
                finish_reason: self.finish_reason.clone(),
                author_name: m.author_name.clone(),
                role: m.role.clone(),
                // Go's zero time is 0001-01-01, distinct from a real Unix-epoch time.
                created_at: m
                    .created_at
                    .filter(|t| t.timestamp() != -62_135_596_800 || t.timestamp_subsec_nanos() != 0)
                    .or(self.created_at),
                contents: m.contents.clone(),
                ..ResponseUpdate::default()
            })
            .collect();
        if !self.additional_properties.is_empty() || !self.continuation_token.is_empty() {
            updates.push(ResponseUpdate {
                additional_properties: self.additional_properties.clone(),
                agent_id: self.agent_id.clone(),
                response_id: self.id.clone(),
                conversation_id: self.conversation_id.clone(),
                continuation_token: self.continuation_token.clone(),
                created_at: self.created_at,
                ..ResponseUpdate::default()
            });
        }
        updates
    }
}
