use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    hash::{BuildHasherDefault, Hasher},
    sync::Arc,
};

pub(crate) type AnyMap =
    HashMap<TypeId, Box<dyn AnyClone + Send + Sync>, BuildHasherDefault<IdHasher>>;

// With TypeIds as keys, there's no need to hash them. They are already hashes
// themselves, coming from the compiler. The IdHasher just holds the u64 of
// the TypeId, and then returns it, instead of doing any bit fiddling.
#[derive(Default)]
pub(crate) struct IdHasher(u64);

impl Hasher for IdHasher {
    fn write(&mut self, _: &[u8]) {
        unreachable!("TypeId calls write_u64");
    }

    #[inline]
    fn write_u64(&mut self, id: u64) {
        self.0 = id;
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// A type map of worker extensions.
///
/// `Extensions` can be used by worker job to store extra data.
#[derive(Clone, Default, Debug)]
pub struct Extensions {
    // If extensions are never used, no need to carry around an empty HashMap.
    // That's 3 words. Instead, this is only 1 word.
    map: Option<Box<AnyMap>>,
}

impl Extensions {
    /// Create an empty `Extensions`.
    #[inline]
    pub fn new() -> Extensions {
        Extensions { map: None }
    }

    /// Insert a type into this `Extensions`.
    ///
    /// If a extension of this type already existed, it will
    /// be returned.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// assert!(ext.insert(5i32).is_none());
    /// assert!(ext.insert(4u8).is_none());
    /// assert_eq!(ext.insert(9i32), Some(5i32));
    /// ```
    pub fn insert<T: Clone + Send + Sync + Debug + 'static>(&mut self, val: T) -> Option<T> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    /// Get a reference to a type previously inserted on this `Extensions`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// assert!(ext.get::<i32>().is_none());
    /// ext.insert(5i32);
    ///
    /// assert_eq!(ext.get::<i32>(), Some(&5i32));
    /// ```
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.map
            .as_ref()
            .and_then(|map| map.get(&TypeId::of::<T>()))
            .and_then(|boxed| (**boxed).as_any().downcast_ref())
    }

    /// Get a mutable reference to a type previously inserted on this `Extensions`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// ext.insert(String::from("Hello"));
    /// ext.get_mut::<String>().unwrap().push_str(" World");
    ///
    /// assert_eq!(ext.get::<String>().unwrap(), "Hello World");
    /// ```
    pub fn get_mut<T: Send + Sync + 'static>(&mut self) -> Option<&mut T> {
        self.map
            .as_mut()
            .and_then(|map| map.get_mut(&TypeId::of::<T>()))
            .and_then(|boxed| (**boxed).as_any_mut().downcast_mut())
    }

    /// Get a mutable reference to a type, inserting `value` if not already present on this
    /// `Extensions`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// *ext.get_or_insert(1i32) += 2;
    ///
    /// assert_eq!(*ext.get::<i32>().unwrap(), 3);
    /// ```
    pub fn get_or_insert<T: Clone + Send + Sync + Debug + 'static>(&mut self, value: T) -> &mut T {
        self.get_or_insert_with(|| value)
    }

    /// Get a mutable reference to a type, inserting the value created by `f` if not already present
    /// on this `Extensions`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// *ext.get_or_insert_with(|| 1i32) += 2;
    ///
    /// assert_eq!(*ext.get::<i32>().unwrap(), 3);
    /// ```
    pub fn get_or_insert_with<T: Clone + Send + Sync + Debug + 'static, F: FnOnce() -> T>(
        &mut self,
        f: F,
    ) -> &mut T {
        let out = self
            .map
            .get_or_insert_with(Box::default)
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(f()));
        (**out).as_any_mut().downcast_mut().unwrap()
    }

    /// Get a mutable reference to a type, inserting the type's default value if not already present
    /// on this `Extensions`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// *ext.get_or_insert_default::<i32>() += 2;
    ///
    /// assert_eq!(*ext.get::<i32>().unwrap(), 2);
    /// ```
    pub fn get_or_insert_default<T: Default + Clone + Send + Sync + Debug + 'static>(
        &mut self,
    ) -> &mut T {
        self.get_or_insert_with(T::default)
    }

    /// Remove a type from this `Extensions`.
    ///
    /// If a extension of this type existed, it will be returned.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// ext.insert(5i32);
    /// assert_eq!(ext.remove::<i32>(), Some(5i32));
    /// assert!(ext.get::<i32>().is_none());
    /// ```
    pub fn remove<T: Send + Sync + 'static>(&mut self) -> Option<T> {
        self.map
            .as_mut()
            .and_then(|map| map.remove(&TypeId::of::<T>()))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    /// Clear the `Extensions` of all inserted extensions.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// ext.insert(5i32);
    /// ext.clear();
    ///
    /// assert!(ext.get::<i32>().is_none());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        if let Some(ref mut map) = self.map {
            map.clear();
        }
    }

    /// Check whether the extension set is empty or not.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// assert!(ext.is_empty());
    /// ext.insert(5i32);
    /// assert!(!ext.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.as_ref().map_or(true, |map| map.is_empty())
    }

    /// Get the numer of extensions available.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext = Extensions::new();
    /// assert_eq!(ext.len(), 0);
    /// ext.insert(5i32);
    /// assert_eq!(ext.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.map.as_ref().map_or(0, |map| map.len())
    }

    /// Extends `self` with another `Extensions`.
    ///
    /// If an instance of a specific type exists in both, the one in `self` is overwritten with the
    /// one from `other`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphile_worker_extensions::Extensions;
    /// let mut ext_a = Extensions::new();
    /// ext_a.insert(8u8);
    /// ext_a.insert(16u16);
    ///
    /// let mut ext_b = Extensions::new();
    /// ext_b.insert(4u8);
    /// ext_b.insert("hello");
    ///
    /// ext_a.extend(ext_b);
    /// assert_eq!(ext_a.len(), 3);
    /// assert_eq!(ext_a.get::<u8>(), Some(&4u8));
    /// assert_eq!(ext_a.get::<u16>(), Some(&16u16));
    /// assert_eq!(ext_a.get::<&'static str>().copied(), Some("hello"));
    /// ```
    pub fn extend(&mut self, other: Self) {
        if let Some(other) = other.map {
            if let Some(map) = &mut self.map {
                map.extend(*other);
            } else {
                self.map = Some(other);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReadOnlyExtensions(Arc<Extensions>);

impl ReadOnlyExtensions {
    pub fn new(ext: Extensions) -> Self {
        ReadOnlyExtensions(Arc::new(ext))
    }

    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.0.get()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Extensions> for ReadOnlyExtensions {
    fn from(ext: Extensions) -> Self {
        ReadOnlyExtensions::new(ext)
    }
}

pub trait AnyClone: Any + Debug {
    fn clone_box(&self) -> Box<dyn AnyClone + Send + Sync>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: Clone + Send + Sync + Debug + 'static> AnyClone for T {
    fn clone_box(&self) -> Box<dyn AnyClone + Send + Sync> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl Clone for Box<dyn AnyClone + Send + Sync> {
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut ext = Extensions::new();
        assert!(ext.insert(5i32).is_none());
        assert_eq!(ext.get::<i32>(), Some(&5i32));
    }

    #[test]
    fn test_insert_and_get_mut() {
        let mut ext = Extensions::new();
        ext.insert(String::from("Hello"));
        ext.get_mut::<String>().unwrap().push_str(" World");
        assert_eq!(ext.get::<String>().unwrap(), "Hello World");
    }

    #[test]
    fn test_get_or_insert() {
        let mut ext = Extensions::new();
        *ext.get_or_insert(1i32) += 2;
        assert_eq!(ext.get::<i32>(), Some(&3i32));
    }

    #[test]
    fn test_get_or_insert_with() {
        let mut ext = Extensions::new();
        *ext.get_or_insert_with(|| 1i32) += 2;
        assert_eq!(ext.get::<i32>(), Some(&3i32));
    }

    #[test]
    fn test_get_or_insert_default() {
        let mut ext = Extensions::new();
        *ext.get_or_insert_default::<i32>() += 2;
        assert_eq!(ext.get::<i32>(), Some(&2i32));
    }

    #[test]
    fn test_remove() {
        let mut ext = Extensions::new();
        ext.insert(5i32);
        assert_eq!(ext.remove::<i32>(), Some(5i32));
        assert!(ext.get::<i32>().is_none());
    }

    #[test]
    fn test_clear() {
        let mut ext = Extensions::new();
        ext.insert(5i32);
        ext.clear();
        assert!(ext.get::<i32>().is_none());
    }

    #[test]
    fn test_is_empty() {
        let mut ext = Extensions::new();
        assert!(ext.is_empty());
        ext.insert(5i32);
        assert!(!ext.is_empty());
    }

    #[test]
    fn test_len() {
        let mut ext = Extensions::new();
        assert_eq!(ext.len(), 0);
        ext.insert(5i32);
        assert_eq!(ext.len(), 1);
    }

    #[test]
    fn test_extend() {
        let mut ext_a = Extensions::new();
        ext_a.insert(8u8);
        ext_a.insert(16u16);

        let mut ext_b = Extensions::new();
        ext_b.insert(4u8);
        ext_b.insert("hello");

        ext_a.extend(ext_b);
        assert_eq!(ext_a.len(), 3);
        assert_eq!(ext_a.get::<u8>(), Some(&4u8));
        assert_eq!(ext_a.get::<u16>(), Some(&16u16));
        assert_eq!(ext_a.get::<&'static str>().copied(), Some("hello"));
    }
}
