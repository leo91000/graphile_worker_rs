use std::sync::Arc;

use crate::Extensions;

/// A read-only wrapper around `Extensions` that can be safely shared.
///
/// `ReadOnlyExtensions` wraps an `Extensions` instance in an `Arc` to allow
/// sharing it between components without allowing modifications. This is used
/// to provide task handlers with access to shared application state without
/// allowing them to modify that state.
#[derive(Clone, Debug)]
pub struct ReadOnlyExtensions(Arc<Extensions>);

impl ReadOnlyExtensions {
    /// Creates a new `ReadOnlyExtensions` from an `Extensions` instance.
    ///
    /// This wraps the provided extensions in an `Arc` to allow safe sharing
    /// across threads and components.
    ///
    /// # Arguments
    ///
    /// * `ext` - The extensions to wrap
    ///
    /// # Returns
    ///
    /// A new `ReadOnlyExtensions` instance
    pub fn new(ext: Extensions) -> Self {
        ReadOnlyExtensions(Arc::new(ext))
    }

    /// Get a reference to a type previously inserted in the wrapped `Extensions`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of extension to retrieve
    ///
    /// # Returns
    ///
    /// * `Some(&T)` - A reference to the extension value if it exists
    /// * `None` - If no extension of the requested type is registered
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.0.get()
    }

    /// Get the number of extensions available.
    ///
    /// # Returns
    ///
    /// The number of extension values stored in this container
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check whether the extension set is empty or not.
    ///
    /// # Returns
    ///
    /// `true` if no extensions are stored, `false` otherwise
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Implements the `From` trait to allow converting an `Extensions` into a
/// `ReadOnlyExtensions` easily.
impl From<Extensions> for ReadOnlyExtensions {
    /// Converts an `Extensions` into a `ReadOnlyExtensions`.
    ///
    /// This is a convenience implementation that simply calls `ReadOnlyExtensions::new`.
    ///
    /// # Arguments
    ///
    /// * `ext` - The extensions to convert
    ///
    /// # Returns
    ///
    /// A new `ReadOnlyExtensions` instance containing the same extensions
    fn from(ext: Extensions) -> Self {
        ReadOnlyExtensions::new(ext)
    }
}
