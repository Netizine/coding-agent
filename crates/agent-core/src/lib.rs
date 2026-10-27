//! Runtime-independent contracts for the Rust Agent Framework.
//!
//! This foundation does not perform network requests or execute tools.
pub mod coalesce;
pub mod content;
pub mod error;
pub mod filter;
pub mod id;
pub mod message;
pub mod raw;
pub mod response;
pub mod strict_schema;
pub mod tool;

pub use content::{Annotation, Content, ContentBody, ContentHeader, Usage};
pub use error::{AgentError, ErrorKind};
pub use message::{Message, Role, Source};
pub use response::{Response, ResponseUpdate};
pub use tool::{ModelRequest, ProviderCapabilities, RunOptions, ToolDescriptor};
