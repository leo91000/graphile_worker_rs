use std::any::TypeId;
use std::fmt::Debug;

use super::Extensions;

impl Extensions {
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
