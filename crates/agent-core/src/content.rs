//! Content uses the reference's discriminators, retaining unknown JSON objects.
use std::collections::BTreeMap;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::AgentError;

pub type Properties = Map<String, Value>;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Usage {
    pub input_token_count: i64,
    pub output_token_count: i64,
    pub total_token_count: i64,
    pub cached_input_token_count: i64,
    pub reasoning_token_count: i64,
    pub additional_counts: BTreeMap<String, i64>,
}

impl Usage {
    /// Saturates malformed/overflowing provider counters instead of panicking.
    pub fn add(&mut self, other: &Self) {
        self.input_token_count = self
            .input_token_count
            .saturating_add(other.input_token_count);
        self.output_token_count = self
            .output_token_count
            .saturating_add(other.output_token_count);
        self.total_token_count = self
            .total_token_count
            .saturating_add(other.total_token_count);
        self.cached_input_token_count = self
            .cached_input_token_count
            .saturating_add(other.cached_input_token_count);
        self.reasoning_token_count = self
            .reasoning_token_count
            .saturating_add(other.reasoning_token_count);
        for (key, value) in &other.additional_counts {
            let count = self.additional_counts.entry(key.clone()).or_default();
            *count = count.saturating_add(*value);
        }
    }
}

/// Explicit distinction between an omitted field, JSON null, and a value.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Field<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<T> Field<T> {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl<T: Serialize> Serialize for Field<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Null | Self::Missing => serializer.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

/// Citation or an unknown annotation, with lossless JSON-value round trips.
#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Citation(Citation),
    Raw(Value),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Citation {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "FileID")]
    pub file_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "URL")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotated_regions: Vec<AnnotatedRegion>,
    #[serde(flatten)]
    pub extra: Properties,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotatedRegion {
    TextSpan(TextSpan),
    Raw(Value),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TextSpan {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub start_index: Field<i64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub end_index: Field<i64>,
    #[serde(flatten)]
    pub extra: Properties,
}

macro_rules! tagged_extension {
    ($enum:ident, $variant:ident, $payload:ty, $tag:literal) => {
        impl Serialize for $enum {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                match self {
                    Self::$variant(value) => {
                        let mut value =
                            serde_json::to_value(value).map_err(serde::ser::Error::custom)?;
                        value["Type"] = Value::String($tag.into());
                        value.serialize(serializer)
                    }
                    Self::Raw(value) => value.serialize(serializer),
                }
            }
        }
        impl<'de> Deserialize<'de> for $enum {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let mut value = Value::deserialize(deserializer)?;
                if !value.is_object() {
                    return Err(serde::de::Error::custom("expected object"));
                }
                if value.get("Type").and_then(Value::as_str) == Some($tag) {
                    value
                        .as_object_mut()
                        .expect("checked object")
                        .remove("Type");
                    let data: $payload =
                        serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                    Ok(Self::$variant(data))
                } else {
                    Ok(Self::Raw(value))
                }
            }
        }
    };
}
tagged_extension!(Annotation, Citation, Citation, "citation");
tagged_extension!(AnnotatedRegion, TextSpan, TextSpan, "textSpan");

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ContentHeader {
    #[serde(skip)]
    pub raw: crate::raw::RawRepresentations,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub additional_properties: Properties,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

/// Standard content plus extension fields on known variants.
#[derive(Clone, Debug, PartialEq)]
pub struct Content {
    pub header: ContentHeader,
    pub body: ContentBody,
    pub extra: Properties,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "Type")]
#[non_exhaustive]
pub enum ContentBody {
    #[serde(rename = "text")]
    Text {
        #[serde(rename = "Text")]
        text: String,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        #[serde(rename = "Text")]
        text: String,
        #[serde(
            rename = "ProtectedData",
            default,
            skip_serializing_if = "String::is_empty"
        )]
        protected_data: String,
    },
    #[serde(rename = "data")]
    Data {
        #[serde(rename = "URI")]
        uri: String,
    },
    #[serde(rename = "uri")]
    Uri {
        #[serde(rename = "URI")]
        uri: String,
        #[serde(rename = "MediaType", default)]
        media_type: String,
    },
    #[serde(rename = "functionCall")]
    FunctionCall {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Arguments")]
        arguments: Value,
    },
    #[serde(rename = "functionResult")]
    FunctionResult {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Result")]
        result: Value,
    },
    #[serde(rename = "usage")]
    Usage {
        #[serde(rename = "Details")]
        details: Usage,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(rename = "Message")]
        message: String,
    },
    #[serde(rename = "hostedFile")]
    HostedFile {
        #[serde(rename = "FileID")]
        file_id: String,
    },
    #[serde(rename = "hostedVectorStore")]
    HostedVectorStore {
        #[serde(rename = "VectorStoreID")]
        vector_store_id: String,
    },
    #[serde(rename = "toolApprovalRequest")]
    ApprovalRequest {
        #[serde(rename = "RequestID")]
        request_id: String,
        #[serde(rename = "ToolCall", default)]
        tool_call: Option<Box<Content>>,
    },
    #[serde(rename = "toolApprovalResponse")]
    ApprovalResponse {
        #[serde(rename = "RequestID")]
        request_id: String,
        #[serde(rename = "Approved")]
        approved: bool,
        #[serde(rename = "Reason", default, skip_serializing_if = "String::is_empty")]
        reason: String,
        #[serde(rename = "ToolCall", default)]
        tool_call: Option<Box<Content>>,
    },
    #[serde(rename = "alwaysApproveToolApprovalResponse")]
    AlwaysApprove {
        #[serde(rename = "innerResponse", default)]
        inner_response: Option<Box<Content>>,
        #[serde(
            rename = "AlwaysApproveTool",
            default,
            skip_serializing_if = "std::ops::Not::not"
        )]
        always_approve_tool: bool,
        #[serde(
            rename = "AlwaysApproveToolWithArguments",
            default,
            skip_serializing_if = "std::ops::Not::not"
        )]
        always_approve_tool_with_arguments: bool,
    },
    #[serde(rename = "mcpServerToolCall")]
    McpCall {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Arguments")]
        arguments: String,
        #[serde(
            rename = "ServerName",
            default,
            skip_serializing_if = "String::is_empty"
        )]
        server_name: String,
    },
    #[serde(rename = "mcpServerToolResult")]
    McpResult {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Outputs", default, skip_serializing_if = "Vec::is_empty")]
        outputs: Vec<Content>,
    },
    #[serde(rename = "codeInterpreterToolCall")]
    CodeCall {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Inputs", default)]
        inputs: Vec<Content>,
    },
    #[serde(rename = "codeInterpreterToolResult")]
    CodeResult {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Outputs", default)]
        outputs: Vec<Content>,
    },
    #[serde(rename = "imageGenerationToolCall")]
    ImageCall {
        #[serde(rename = "CallID")]
        call_id: String,
    },
    #[serde(rename = "imageGenerationToolResult")]
    ImageResult {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Outputs", default)]
        outputs: Vec<Content>,
    },
    #[serde(rename = "webSearchToolCall")]
    WebCall {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Queries", default)]
        queries: Vec<String>,
    },
    #[serde(rename = "webSearchToolResult")]
    WebResult {
        #[serde(rename = "CallID")]
        call_id: String,
        #[serde(rename = "Outputs", default)]
        outputs: Vec<Content>,
    },
    /// Unknown content retains its entire JSON object.
    #[serde(skip)]
    Raw(Value),
}

impl Content {
    pub fn new(body: ContentBody) -> Self {
        Self {
            header: ContentHeader::default(),
            body,
            extra: Properties::new(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new(ContentBody::Text { text: text.into() })
    }
    pub fn reasoning(text: impl Into<String>) -> Self {
        Self::new(ContentBody::Reasoning {
            text: text.into(),
            protected_data: String::new(),
        })
    }

    pub fn data(bytes: &[u8], media_type: &str) -> Result<Self, AgentError> {
        validate_media_type(media_type)?;
        Ok(Self::new(ContentBody::Data {
            uri: format!("data:{media_type};base64,{}", STANDARD.encode(bytes)),
        }))
    }

    pub fn uri(uri: impl Into<String>, media_type: impl Into<String>) -> Result<Self, AgentError> {
        let uri = uri.into();
        url::Url::parse(&uri).map_err(|_| AgentError::validation("URI must be absolute"))?;
        let media_type = media_type.into();
        if !media_type.is_empty() {
            validate_media_type(&media_type)?;
        }
        Ok(Self::new(ContentBody::Uri { uri, media_type }))
    }

    pub fn as_text(&self) -> Option<&str> {
        if let ContentBody::Text { text } = &self.body {
            Some(text)
        } else {
            None
        }
    }

    pub fn call_id(&self) -> Option<&str> {
        match &self.body {
            ContentBody::FunctionCall { call_id, .. }
            | ContentBody::McpCall { call_id, .. }
            | ContentBody::CodeCall { call_id, .. }
            | ContentBody::ImageCall { call_id }
            | ContentBody::WebCall { call_id, .. } => Some(call_id),
            _ => None,
        }
    }

    fn validate_approval(&self) -> Result<(), AgentError> {
        match &self.body {
            ContentBody::ApprovalRequest { tool_call, .. }
            | ContentBody::ApprovalResponse { tool_call, .. } => {
                if tool_call.as_ref().is_some_and(|c| c.call_id().is_none()) {
                    return Err(AgentError::validation(
                        "approval must reference a supported tool call",
                    ));
                }
            }
            ContentBody::AlwaysApprove {
                inner_response: Some(inner),
                ..
            } if !matches!(inner.body, ContentBody::ApprovalResponse { .. }) => {
                return Err(AgentError::validation(
                    "standing approval must wrap an approval response",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

fn fields(tag: &str) -> Option<&'static [&'static str]> {
    Some(match tag {
        "text" => &["Text"],
        "reasoning" => &["Text", "ProtectedData"],
        "data" => &["URI"],
        "uri" => &["URI", "MediaType"],
        "functionCall" => &["CallID", "Name", "Arguments"],
        "functionResult" => &["CallID", "Result"],
        "usage" => &["Details"],
        "error" => &["Message"],
        "hostedFile" => &["FileID"],
        "hostedVectorStore" => &["VectorStoreID"],
        "toolApprovalRequest" => &["RequestID", "ToolCall"],
        "toolApprovalResponse" => &["RequestID", "Approved", "Reason", "ToolCall"],
        "alwaysApproveToolApprovalResponse" => &[
            "innerResponse",
            "AlwaysApproveTool",
            "AlwaysApproveToolWithArguments",
        ],
        "mcpServerToolCall" => &["CallID", "Name", "Arguments", "ServerName"],
        "codeInterpreterToolCall" => &["CallID", "Inputs"],
        "imageGenerationToolCall" => &["CallID"],
        "webSearchToolCall" => &["CallID", "Queries"],
        "mcpServerToolResult"
        | "codeInterpreterToolResult"
        | "imageGenerationToolResult"
        | "webSearchToolResult" => &["CallID", "Outputs"],
        _ => return None,
    })
}

impl Serialize for Content {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate_approval()
            .map_err(serde::ser::Error::custom)?;
        if let ContentBody::Raw(value) = &self.body {
            return value.serialize(serializer);
        }
        let mut object = self.extra.clone();
        let header = serde_json::to_value(&self.header).map_err(serde::ser::Error::custom)?;
        object.extend(header.as_object().expect("header object").clone());
        let body = serde_json::to_value(&self.body).map_err(serde::ser::Error::custom)?;
        object.extend(body.as_object().expect("body object").clone());
        if let ContentBody::Data { uri } = &self.body {
            decode_data_uri(uri, usize::MAX).map_err(serde::ser::Error::custom)?;
        }
        object.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let mut extra = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("content must be an object"))?
            .clone();
        let tag = value.get("Type").and_then(Value::as_str).unwrap_or("");
        let Some(known_fields) = fields(tag) else {
            return Ok(Self::new(ContentBody::Raw(value)));
        };
        let body: ContentBody =
            serde_json::from_value(value.clone()).map_err(serde::de::Error::custom)?;
        if let ContentBody::Data { uri } = &body {
            decode_data_uri(uri, usize::MAX).map_err(serde::de::Error::custom)?;
        }
        if let ContentBody::Uri { uri, media_type } = &body {
            Self::uri(uri, media_type).map_err(serde::de::Error::custom)?;
        }
        let header = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        for field in
            known_fields
                .iter()
                .copied()
                .chain(["Type", "Annotations", "AdditionalProperties"])
        {
            extra.remove(field);
        }
        let content = Self {
            header,
            body,
            extra,
        };
        content
            .validate_approval()
            .map_err(serde::de::Error::custom)?;
        Ok(content)
    }
}

fn validate_media_type(media_type: &str) -> Result<(), AgentError> {
    media_type
        .parse::<mime::Mime>()
        .map(|_| ())
        .map_err(|_| AgentError::validation("invalid media type"))
}

/// Decode a data URI without fetching resources. `max_bytes` bounds decoded output.
pub fn decode_data_uri(uri: &str, max_bytes: usize) -> Result<(Vec<u8>, String), AgentError> {
    if !uri
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        return Err(AgentError::validation("expected data URI"));
    }
    let (metadata, encoded) = uri[5..]
        .split_once(',')
        .ok_or_else(|| AgentError::validation("data URI needs a comma"))?;
    let base64 = metadata.to_ascii_lowercase().ends_with(";base64");
    let media_type = if base64 {
        &metadata[..metadata.len() - 7]
    } else {
        metadata
    };
    let media_type = if media_type.trim().is_empty() {
        "text/plain;charset=US-ASCII"
    } else {
        media_type.trim()
    };
    validate_media_type(media_type)?;
    let bytes = if base64 {
        // Reject oversized input before allocating its decoded form.
        if encoded.len() / 4 * 3 > max_bytes.saturating_add(2) {
            return Err(AgentError::validation("data exceeds limit"));
        }
        STANDARD
            .decode(encoded)
            .map_err(|_| AgentError::validation("invalid base64 data"))?
    } else {
        let mut bytes = Vec::new();
        let mut iter = encoded.bytes();
        while let Some(byte) = iter.next() {
            let decoded = if byte == b'%' {
                let high = iter.next().and_then(|b| (b as char).to_digit(16));
                let low = iter.next().and_then(|b| (b as char).to_digit(16));
                match (high, low) {
                    (Some(h), Some(l)) => (h * 16 + l) as u8,
                    _ => return Err(AgentError::validation("invalid percent encoding")),
                }
            } else {
                byte
            };
            if bytes.len() >= max_bytes {
                return Err(AgentError::validation("data exceeds limit"));
            }
            bytes.push(decoded);
        }
        bytes
    };
    if bytes.len() > max_bytes {
        return Err(AgentError::validation("data exceeds limit"));
    }
    Ok((bytes, media_type.into()))
}
