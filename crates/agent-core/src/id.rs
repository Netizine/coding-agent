use crate::AgentError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, marker::PhantomData};

/// Nonempty, domain-typed identifier for APIs that require identity.
/// Wire response fields remain strings because Go allows absent/empty identities.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id<Domain> {
    value: String,
    domain: PhantomData<Domain>,
}
impl<D> Id<D> {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(AgentError::validation(
                "identifier must be nonblank and contain no control characters",
            ));
        }
        Ok(Self {
            value,
            domain: PhantomData,
        })
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
}
impl<D> fmt::Display for Id<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}
impl<D> Serialize for Id<D> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(s)
    }
}
impl<'de, D> Deserialize<'de> for Id<D> {
    fn deserialize<T: Deserializer<'de>>(d: T) -> Result<Self, T::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentIdentity {}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SessionIdentity {}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallIdentity {}
pub type AgentId = Id<AgentIdentity>;
pub type SessionId = Id<SessionIdentity>;
pub type CallId = Id<CallIdentity>;
