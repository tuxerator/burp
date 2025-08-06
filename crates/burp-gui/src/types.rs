use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};

pub(crate) struct Dirty<T> {
    inner: T,
    dirty: bool,
}

impl<T> Dirty<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self {
            inner,
            dirty: false,
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn set_clean(&mut self) {
        self.dirty = false;
    }

    pub(crate) fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Deref for Dirty<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for Dirty<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dirty = true;
        &mut self.inner
    }
}

impl<T: Serialize> Serialize for Dirty<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Dirty<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::new(T::deserialize(deserializer)?))
    }
}
