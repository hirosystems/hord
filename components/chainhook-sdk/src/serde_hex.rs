
use bitcoin::hex::{DisplayHex, FromHex};
use serde::de::Error;
use serde::{Deserializer, Serializer};

pub fn serialize<S: Serializer>(b: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
  s.serialize_str(&b.to_lower_hex_string())
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
  let hex_str: String = ::serde::Deserialize::deserialize(d)?;
  FromHex::from_hex(&hex_str).map_err(D::Error::custom)
}
