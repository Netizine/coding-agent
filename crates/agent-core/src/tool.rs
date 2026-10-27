use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{AgentError, Content, ContentBody, ErrorKind, Message, content::Field};

/// Validated, self-contained JSON schema. Remote/file reference fetching is disabled.
pub struct Schema {
    value: Value,
    validator: jsonschema::Validator,
}

impl Schema {
    pub fn new(value: Value) -> Result<Self, AgentError> {
        let validator = jsonschema::validator_for(&value)
            .map_err(|_| AgentError::validation("invalid or unresolved JSON schema"))?;
        Ok(Self { value, validator })
    }

    pub fn for_type<T: JsonSchema>() -> Result<Self, AgentError> {
        Self::new(
            serde_json::to_value(schemars::schema_for!(T))
                .map_err(|_| AgentError::validation("schema generation failed"))?,
        )
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn validate(&self, value: &Value) -> Result<(), AgentError> {
        if self.validator.is_valid(value) {
            Ok(())
        } else {
            Err(AgentError::validation("value does not match schema"))
        }
    }

    pub fn decode<T: DeserializeOwned>(&self, value: Value) -> Result<T, AgentError> {
        self.validate(&value)?;
        serde_json::from_value(value)
            .map_err(|_| AgentError::validation("value does not match Rust type"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub approval_required: bool,
}

impl ToolDescriptor {
    pub fn typed<T: JsonSchema>(
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, AgentError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(AgentError::validation("tool name is blank"));
        }
        Ok(Self {
            name,
            description: description.into(),
            input_schema: Schema::for_type::<T>()?.value,
            approval_required: false,
        })
    }

    pub fn validate(&self) -> Result<(), AgentError> {
        if self.name.trim().is_empty() {
            return Err(AgentError::validation("tool name is blank"));
        }
        Schema::new(self.input_schema.clone())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunOptions {
    pub stream: bool,
    pub instructions: Option<String>,
    pub service_id: Option<String>,
    pub continuation_token: Option<String>,
    pub allow_background: bool,
    pub output_schema: Option<Value>,
    /// None selects the runtime default; zero explicitly disables auto-calls.
    pub max_tool_iterations: Option<u32>,
    pub tool_mode: ToolMode,
    pub parallel_tool_calls: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    #[default]
    Auto,
    None,
    Required,
    Named(String),
}

/// Applied from left to right. Missing retains a value, null clears an optional value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunOptionsPatch {
    pub stream: Option<bool>,
    pub allow_background: Option<bool>,
    pub parallel_tool_calls: Option<bool>,
    pub tool_mode: Option<ToolMode>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub instructions: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub service_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub continuation_token: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub output_schema: Field<Value>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub max_tool_iterations: Field<u32>,
}

impl RunOptions {
    pub fn overlay(&mut self, patch: &RunOptionsPatch) {
        fn optional<T: Clone>(value: &mut Option<T>, patch: &Field<T>) {
            match patch {
                Field::Missing => {}
                Field::Null => *value = None,
                Field::Value(v) => *value = Some(v.clone()),
            }
        }
        if let Some(v) = patch.stream {
            self.stream = v;
        }
        if let Some(v) = patch.allow_background {
            self.allow_background = v;
        }
        if let Some(v) = patch.parallel_tool_calls {
            self.parallel_tool_calls = v;
        }
        if let Some(v) = &patch.tool_mode {
            self.tool_mode = v.clone();
        }
        optional(&mut self.instructions, &patch.instructions);
        optional(&mut self.service_id, &patch.service_id);
        optional(&mut self.continuation_token, &patch.continuation_token);
        optional(&mut self.output_schema, &patch.output_schema);
        optional(&mut self.max_tool_iterations, &patch.max_tool_iterations);
    }
    pub fn effective_tool_iterations(&self) -> u32 {
        self.max_tool_iterations.unwrap_or(40)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDescriptor>,
    pub options: RunOptions,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionOwner {
    #[default]
    Framework,
    Provider,
}

/// Conservative capability declaration; unsupported requests must fail explicitly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tools: bool,
    pub structured_output: bool,
    pub service_history: bool,
    pub background: bool,
    pub tool_execution_owner: ToolExecutionOwner,
    pub parallel_tools: bool,
    pub reasoning: bool,
    pub images: bool,
    pub audio: bool,
    pub files: bool,
    pub provider_extensions: bool,
    pub hosted_tools: bool,
}

impl ProviderCapabilities {
    pub fn validate(&self, request: &ModelRequest) -> Result<(), AgentError> {
        for (needed, supported, name) in [
            (request.options.stream, self.streaming, "streaming"),
            (!request.tools.is_empty(), self.tools, "tools"),
            (
                request.options.parallel_tool_calls,
                self.parallel_tools,
                "parallel tools",
            ),
            (
                request.options.output_schema.is_some(),
                self.structured_output,
                "structured output",
            ),
            (
                request.options.service_id.is_some(),
                self.service_history,
                "service history",
            ),
            (
                request.options.allow_background || request.options.continuation_token.is_some(),
                self.background,
                "background/continuation",
            ),
        ] {
            if needed && !supported {
                return Err(AgentError::new(ErrorKind::UnsupportedCapability, name));
            }
        }
        for tool in &request.tools {
            tool.validate()?;
        }
        let mut names = std::collections::BTreeSet::new();
        for tool in &request.tools {
            if !names.insert(&tool.name) {
                return Err(AgentError::validation("duplicate tool name"));
            }
        }
        match &request.options.tool_mode {
            ToolMode::Required if request.tools.is_empty() => {
                return Err(AgentError::validation("required tool mode needs tools"));
            }
            ToolMode::Named(name) if !names.contains(name) => {
                return Err(AgentError::validation("named tool is not registered"));
            }
            _ => {}
        }
        for message in &request.messages {
            for content in &message.contents {
                self.validate_content(content)?;
            }
        }
        if let Some(schema) = &request.options.output_schema {
            Schema::new(schema.clone())?;
        }
        Ok(())
    }

    fn validate_content(&self, content: &Content) -> Result<(), AgentError> {
        let (supported, label) = match &content.body {
            ContentBody::Data { uri } => {
                let (_, mime) = crate::content::decode_data_uri(uri, usize::MAX)?;
                if mime.starts_with("image/") {
                    (self.images, "images")
                } else if mime.starts_with("audio/") {
                    (self.audio, "audio")
                } else {
                    (self.files, "files")
                }
            }
            ContentBody::Uri { media_type, .. } => {
                if media_type.starts_with("image/") {
                    (self.images, "images")
                } else if media_type.starts_with("audio/") {
                    (self.audio, "audio")
                } else {
                    (self.files, "files")
                }
            }
            ContentBody::Reasoning { .. } => (self.reasoning, "reasoning"),
            ContentBody::Raw(_) => (self.provider_extensions, "provider extensions"),
            ContentBody::HostedFile { .. } | ContentBody::HostedVectorStore { .. } => {
                (self.files, "hosted files")
            }
            ContentBody::McpCall { .. }
            | ContentBody::McpResult { .. }
            | ContentBody::CodeCall { .. }
            | ContentBody::CodeResult { .. }
            | ContentBody::ImageCall { .. }
            | ContentBody::ImageResult { .. }
            | ContentBody::WebCall { .. }
            | ContentBody::WebResult { .. } => (self.hosted_tools, "hosted tools"),
            _ => (true, "content"),
        };
        if !supported {
            return Err(AgentError::new(ErrorKind::UnsupportedCapability, label));
        }
        match &content.body {
            ContentBody::CodeCall { inputs, .. } => {
                for c in inputs {
                    self.validate_content(c)?;
                }
            }
            ContentBody::McpResult { outputs, .. }
            | ContentBody::CodeResult { outputs, .. }
            | ContentBody::ImageResult { outputs, .. }
            | ContentBody::WebResult { outputs, .. } => {
                for c in outputs {
                    self.validate_content(c)?;
                }
            }
            ContentBody::ApprovalRequest {
                tool_call: Some(c), ..
            }
            | ContentBody::ApprovalResponse {
                tool_call: Some(c), ..
            }
            | ContentBody::AlwaysApprove {
                inner_response: Some(c),
                ..
            } => self.validate_content(c)?,
            _ => {}
        }
        Ok(())
    }
}
