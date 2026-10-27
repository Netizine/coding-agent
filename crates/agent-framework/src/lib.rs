//! Rust Agent Framework foundation.
//!
//! ```
//! use agent_framework::Message;
//! let message = Message::user("Hello");
//! assert_eq!(message.text(), "Hello");
//! ```
pub use agent_core as core;
pub use agent_core::*;
#[cfg(feature = "runtime")]
pub use agent_runtime as runtime;
