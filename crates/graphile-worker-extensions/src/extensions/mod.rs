mod access;
mod mutation;

use crate::map::AnyMap;

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
}
