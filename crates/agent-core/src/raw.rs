//! Runtime-only provider objects: cloned by shared ownership, never serialized or logged.
use std::{any::Any, fmt, sync::Arc};

#[derive(Clone, Default)]
pub struct RawRepresentations(Vec<Arc<dyn Any + Send + Sync>>);

impl RawRepresentations {
    pub fn push<T: Any + Send + Sync>(&mut self, value: T) {
        self.0.push(Arc::new(value));
    }
    pub fn get<T: Any>(&self, index: usize) -> Option<&T> {
        self.0.get(index)?.downcast_ref()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn append(&mut self, other: &Self) {
        self.0.extend(other.0.iter().cloned());
    }
}
impl fmt::Debug for RawRepresentations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawRepresentations")
            .field("count", &self.len())
            .finish()
    }
}
impl PartialEq for RawRepresentations {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().zip(&other.0).all(|(a, b)| Arc::ptr_eq(a, b))
    }
}
