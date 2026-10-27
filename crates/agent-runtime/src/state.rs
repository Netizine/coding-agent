//! P0 persistence spike: explicit schema identity, no Rust TypeId/type-name serialization.
//! This is a value envelope, not a workflow checkpoint or session store.
use agent_core::{AgentError, ErrorKind};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateEnvelope {
    pub format_version: u32,
    pub schema_id: String,
    pub schema_version: u32,
    pub value: Value,
}

impl StateEnvelope {
    pub fn encode<T: Serialize>(
        schema_id: &str,
        schema_version: u32,
        value: &T,
    ) -> Result<Self, AgentError> {
        if schema_id.trim().is_empty() || schema_version == 0 {
            return Err(AgentError::validation(
                "state requires a stable schema ID and nonzero version",
            ));
        }
        Ok(Self {
            format_version: 1,
            schema_id: schema_id.into(),
            schema_version,
            value: serde_json::to_value(value)
                .map_err(|_| AgentError::new(ErrorKind::CorruptState, "cannot encode state"))?,
        })
    }

    pub fn decode<T: DeserializeOwned>(
        &self,
        schema_id: &str,
        schema_version: u32,
    ) -> Result<T, AgentError> {
        if self.format_version != 1
            || schema_id.trim().is_empty()
            || schema_version == 0
            || self.schema_id != schema_id
            || self.schema_version != schema_version
        {
            return Err(AgentError::new(
                ErrorKind::IncompatibleSchema,
                "state format or schema does not match",
            ));
        }
        serde_json::from_value(self.value.clone())
            .map_err(|_| AgentError::new(ErrorKind::CorruptState, "invalid state payload"))
    }
}

/// Explicit registration of durable type/schema pairs. No compiler type name is persisted.
#[derive(Default)]
pub struct CodecRegistry {
    codecs: std::collections::BTreeMap<(String, u32), Codec>,
}
type AnyValue = Box<dyn std::any::Any + Send + Sync>;
struct Codec {
    type_id: std::any::TypeId,
    decode: fn(Value) -> Result<AnyValue, AgentError>,
}
impl CodecRegistry {
    pub fn register<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        id: &str,
        version: u32,
    ) -> Result<(), AgentError> {
        if id.trim().is_empty() || version == 0 {
            return Err(AgentError::validation("invalid codec identity"));
        }
        let key = (id.to_owned(), version);
        if self.codecs.contains_key(&key) {
            return Err(AgentError::validation("codec already registered"));
        }
        self.codecs.insert(
            key,
            Codec {
                type_id: std::any::TypeId::of::<T>(),
                decode: |v| {
                    serde_json::from_value::<T>(v)
                        .map(|v| Box::new(v) as AnyValue)
                        .map_err(|_| {
                            AgentError::new(ErrorKind::CorruptState, "invalid registered payload")
                        })
                },
            },
        );
        Ok(())
    }
    pub fn decode<T: Send + Sync + 'static>(
        &self,
        envelope: &StateEnvelope,
    ) -> Result<T, AgentError> {
        if envelope.format_version != 1 {
            return Err(AgentError::new(
                ErrorKind::IncompatibleSchema,
                "unknown envelope version",
            ));
        }
        let codec = self
            .codecs
            .get(&(envelope.schema_id.clone(), envelope.schema_version))
            .ok_or_else(|| AgentError::new(ErrorKind::IncompatibleSchema, "unregistered schema"))?;
        if codec.type_id != std::any::TypeId::of::<T>() {
            return Err(AgentError::new(
                ErrorKind::IncompatibleSchema,
                "registered type mismatch",
            ));
        }
        (codec.decode)(envelope.value.clone())?
            .downcast::<T>()
            .map(|v| *v)
            .map_err(|_| AgentError::new(ErrorKind::IncompatibleSchema, "codec type mismatch"))
    }
}
