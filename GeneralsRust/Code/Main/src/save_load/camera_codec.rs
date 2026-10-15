//! Bounded fixed-integer encoding for the versioned camera owner records.
//! Main's legacy API shim has no allocation limit, so decode directly through
//! bincode 2 and require the entire detached record to be consumed.
use serde::{Serialize, de::DeserializeOwned};

pub(crate) struct CameraCodec<const LIMIT: usize>;
impl<const LIMIT: usize> CameraCodec<LIMIT> {
    pub(crate) fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, String> {
        let bytes = bincode::serde::encode_to_vec(value, bincode::config::legacy())
            .map_err(|error| error.to_string())?;
        if bytes.len() > LIMIT {
            return Err("camera continuation exceeds size limit".into());
        }
        Ok(bytes)
    }
    pub(crate) fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, String> {
        if bytes.len() > LIMIT {
            return Err("camera continuation exceeds size limit".into());
        }
        let (value, consumed) = bincode::serde::decode_from_slice(
            bytes,
            bincode::config::legacy().with_limit::<LIMIT>(),
        )
        .map_err(|error| error.to_string())?;
        if consumed != bytes.len() {
            return Err("camera continuation contains trailing bytes".into());
        }
        Ok(value)
    }
}
