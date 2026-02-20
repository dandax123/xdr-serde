//! # xdr-serde
//!
//! A pure-Rust implementation of XDR (eXternal Data Representation, RFC 4506)
//! serialization and deserialization, built on top of the `serde` framework.
//!
//! ## Overview
//!
//! XDR is the wire encoding used by ONC RPC protocols such as NFS. All values
//! are big-endian (network byte order), and every item occupies a multiple of
//! 4 bytes (padded with zeroes as needed).
//!
//! ## Quick start
//!
//! ```rust
//! use serde::{Deserialize, Serialize};
//! use xdr_serde::{from_bytes, to_bytes};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct FileHandle {
//!     inode: u64,
//!     generation: u32,
//!     flags: u32,
//! }
//!
//! let fh = FileHandle { inode: 0x0102030405060708, generation: 42, flags: 0 };
//!
//! let bytes = to_bytes(&fh).unwrap();
//! assert_eq!(bytes.len(), 16); // 8 + 4 + 4
//!
//! let decoded: FileHandle = from_bytes(&bytes).unwrap();
//! assert_eq!(fh, decoded);
//! ```

pub mod de;
pub mod error;
pub mod fixed_opaque;
pub mod ser;

pub use de::{Deserializer, ReaderDeserializer, from_bytes, from_bytes_partial, from_reader};
pub use error::{Error, Result};
pub use ser::{Serializer, to_bytes, to_writer};
pub use serde::{Deserialize, Serialize};

/// Sentinel name passed to `serialize_newtype_struct` / `deserialize_newtype_struct`
/// so that our XDR serializer can distinguish fixed-length opaque data (no length
/// prefix, raw bytes + padding) from ordinary variable-length opaque data.
///
/// This is an implementation detail; users interact with it only via
/// `#[serde(with = "xdr_serde::fixed_opaque")]`.
pub const FIXED_OPAQUE_TOKEN: &str = "__xdr_fixed_opaque__";
