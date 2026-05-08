//! Serde-based translation helpers used at crate boundaries.

use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) fn translate<T, R>(value: T) -> Result<R, String>
where
    T: Serialize,
    R: DeserializeOwned,
{
    let raw = serde_json::to_value(value).map_err(|err| err.to_string())?;
    serde_json::from_value(raw).map_err(|err| err.to_string())
}
