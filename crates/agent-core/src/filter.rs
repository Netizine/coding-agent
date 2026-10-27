//! Composable removal-only filters preserve order and duplicate input positions.
use crate::{Message, Source};

#[derive(Clone, Debug)]
pub enum MessageFilter {
    All,
    None,
    ExternalOnly,
    Sources(Vec<Source>),
    NotSources(Vec<Source>),
    NotSourceTypes(Vec<String>),
    And(Vec<Self>),
    Or(Vec<Self>),
}

impl MessageFilter {
    pub fn matches(&self, message: &Message) -> bool {
        match self {
            Self::All => true,
            Self::None => false,
            Self::ExternalOnly => message.source.kind.is_empty(),
            Self::Sources(sources) => sources.contains(&message.source),
            Self::NotSources(sources) => !sources.contains(&message.source),
            Self::NotSourceTypes(types) => !types.contains(&message.source.kind),
            Self::And(filters) => filters.iter().all(|f| f.matches(message)),
            Self::Or(filters) => filters.iter().any(|f| f.matches(message)),
        }
    }

    pub fn apply<'a>(&self, messages: &'a [Message]) -> Vec<&'a Message> {
        messages.iter().filter(|m| self.matches(m)).collect()
    }
}
