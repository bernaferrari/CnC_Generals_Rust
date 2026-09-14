//! Classic bincode 1 API on top of bincode 2's `legacy()` config.
//!
//! bincode 3.0.0 is a stub that does not compile. 2.0.1 is the newest working
//! release. `legacy()` matches bincode 1's fixed-int little-endian layout.

use serde::{Serialize, de::DeserializeOwned};

pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type Result<T> = std::result::Result<T, Error>;

fn config() -> impl bincode::config::Config {
    bincode::config::legacy()
}

pub fn serialize<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    bincode::serde::encode_to_vec(value, config()).map_err(|e| Error::from(e.to_string()))
}

pub fn deserialize<'a, T: DeserializeOwned>(bytes: &'a [u8]) -> Result<T> {
    bincode::serde::decode_from_slice(bytes, config())
        .map(|(value, _)| value)
        .map_err(|e| Error::from(e.to_string()))
}

pub trait Options: Sized {
    fn deserialize<T: DeserializeOwned>(self, bytes: &[u8]) -> Result<T>;
    fn serialize<T: Serialize + ?Sized>(self, value: &T) -> Result<Vec<u8>>;
}

#[derive(Default, Clone, Copy)]
pub struct DefaultOptions;

impl DefaultOptions {
    pub fn new() -> Self {
        Self
    }

    pub fn with_fixint_encoding(self) -> Self {
        self
    }

    pub fn allow_trailing_bytes(self) -> Self {
        self
    }

    pub fn reject_trailing_bytes(self) -> Self {
        self
    }
}

impl Options for DefaultOptions {
    fn deserialize<T: DeserializeOwned>(self, bytes: &[u8]) -> Result<T> {
        deserialize(bytes)
    }

    fn serialize<T: Serialize + ?Sized>(self, value: &T) -> Result<Vec<u8>> {
        serialize(value)
    }
}
