//! Serde helper module for XDR fixed-length opaque data (RFC 4506 §4.9).
//!
//! Fixed-length opaque is XDR's term for a raw byte array whose size is known
//! at compile time. Unlike variable-length opaque (which prefixes the count),
//! fixed-length opaque is encoded as the raw bytes followed only by 0–3 zero-
//! padding bytes to align to a 4-byte boundary — **no length prefix**.
//!
//! # Usage
//!
//! Annotate any `[u8; N]` field with `#[serde(with = "xdr_serde::fixed_opaque")]`:
//!
//! ```rust
//! use serde::{Deserialize, Serialize};
//! use xdr_serde::{from_bytes, to_bytes};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! pub struct StateId {
//!     pub sequence_id: u32,
//!     #[serde(with = "xdr_serde::fixed_opaque")]
//!     pub other: [u8; 12],
//! }
//!
//! let id = StateId { sequence_id: 7, other: [1,2,3,4,5,6,7,8,9,10,11,12] };
//!
//! let bytes = to_bytes(&id).unwrap();
//! // 4 bytes (sequence_id) + 12 bytes ([u8;12], no padding as 12%4==0) = 16
//! assert_eq!(bytes.len(), 16);
//! assert_eq!(&bytes[..4], [0, 0, 0, 7]);
//! assert_eq!(&bytes[4..], [1,2,3,4,5,6,7,8,9,10,11,12]);
//!
//! let decoded: StateId = from_bytes(&bytes).unwrap();
//! assert_eq!(id, decoded);
//! ```
//!
//! # Default behaviour without this module
//!
//! Without `#[serde(with = "xdr_serde::fixed_opaque")]`, serde would derive
//! `Serialize` for `[u8; 12]` as a tuple of 12 elements, each serialized as a
//! u8 which in XDR is promoted to a 4-byte unsigned int — **48 bytes total**.
//! This module encodes the same field in just 12 bytes (the XDR spec's intended
//! representation).
//!
//! # Wire format
//!
//! ```text
//! +--------+--------+...+--------+---...---+
//! | byte 0 | byte 1 |...| byte N-1 |  r×0   |
//! +--------+--------+...+--------+---...---+
//! |<-----------N bytes---------->|<--pad--->|
//!                         where (N + r) % 4 == 0
//! ```
//!
//! # Supported types
//!
//! Any type that implements [`XdrFixedOpaque`] can be used. A blanket
//! implementation covers `[u8; N]` for any const `N`.

// ── Sealed trait ──────────────────────────────────────────────────────────

mod private {
    pub trait Sealed {}
}

/// Marker trait for types that can be serialized/deserialized as XDR
/// fixed-length opaque data.
///
/// This trait is sealed — only `[u8; N]` implements it.
pub trait XdrFixedOpaque: private::Sealed + Sized {
    /// The byte length on the wire (before padding).
    fn fixed_len() -> usize;
    /// Borrow the raw bytes.
    fn as_bytes(&self) -> &[u8];
    /// Construct from a slice of exactly `fixed_len()` bytes.
    fn from_exact_bytes(bytes: &[u8]) -> Option<Self>;
}

impl<const N: usize> private::Sealed for [u8; N] {}

impl<const N: usize> XdrFixedOpaque for [u8; N] {
    fn fixed_len() -> usize {
        N
    }
    fn as_bytes(&self) -> &[u8] {
        self.as_slice()
    }
    fn from_exact_bytes(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().ok()
    }
}

// ── serde `with` module functions ─────────────────────────────────────────

/// Serialize `value` as XDR fixed-length opaque: raw bytes + 0–3 padding.
/// No length prefix is written.
pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: XdrFixedOpaque,
    S: serde::Serializer,
{
    // We signal to our XDR serializer (via the FIXED_OPAQUE_TOKEN name) that
    // the inner value should be written as padded raw bytes without a length
    // prefix. The inner FixedOpaqueHelper calls serialize_bytes, which the
    // FixedOpaqueSerializer wrapper routes to write_padded_bytes.
    serializer.serialize_newtype_struct(
        crate::FIXED_OPAQUE_TOKEN,
        &FixedOpaqueHelper(value.as_bytes()),
    )
}

/// Deserialize a fixed-length opaque value: consume exactly N bytes + padding.
/// No length prefix is read.
pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: XdrFixedOpaque + serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_newtype_struct(
        crate::FIXED_OPAQUE_TOKEN,
        FixedOpaqueVisitor::<T>(std::marker::PhantomData),
    )
}

// ── Internal types ─────────────────────────────────────────────────────────

/// Wraps a raw byte slice so `serialize_bytes` is called on it (intercepted
/// by the XDR serializer to write padded bytes without a length prefix).
pub(crate) struct FixedOpaqueHelper<'a>(pub &'a [u8]);

impl serde::Serialize for FixedOpaqueHelper<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}

/// Visitor for deserializing fixed-length opaque.
///
/// When our XDR deserializer sees FIXED_OPAQUE_TOKEN in
/// `deserialize_newtype_struct`, it wraps itself in a `FixedOpaqueSliceDe` /
/// `FixedOpaqueReaderDe` and calls `visit_newtype_struct(inner_de)`. We then
/// drive `inner_de` by calling `T::deserialize(inner_de)`, which for `[u8;N]`
/// calls `inner_de.deserialize_tuple(N, ...)`. The inner deserializer reads N
/// raw bytes (with trailing padding consumed) and yields them as a seq of u8.
struct FixedOpaqueVisitor<T>(std::marker::PhantomData<T>);

impl<'de, T: XdrFixedOpaque + serde::Deserialize<'de>> serde::de::Visitor<'de>
    for FixedOpaqueVisitor<T>
{
    type Value = T;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fixed-length opaque ({} bytes)", T::fixed_len())
    }

    /// Called by our XDR deserializer with an inner `FixedOpaqueSliceDe` /
    /// `FixedOpaqueReaderDe`. We ask `T` to deserialize itself from that inner
    /// deserializer, which will call `deserialize_tuple(N, ...)` and read the
    /// raw bytes with padding.
    fn visit_newtype_struct<D: serde::Deserializer<'de>>(self, de: D) -> Result<T, D::Error> {
        T::deserialize(de)
    }

    // Fallback paths for non-XDR serializers (e.g. JSON or test purposes).
    fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<T, E> {
        T::from_exact_bytes(v).ok_or_else(|| E::invalid_length(v.len(), &self))
    }
    fn visit_byte_buf<E: serde::de::Error>(self, v: Vec<u8>) -> Result<T, E> {
        self.visit_bytes(&v)
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<T, A::Error> {
        let mut buf = Vec::with_capacity(T::fixed_len());
        while let Some(b) = seq.next_element::<u8>()? {
            buf.push(b);
        }
        T::from_exact_bytes(&buf).ok_or_else(|| serde::de::Error::invalid_length(buf.len(), &self))
    }
}
