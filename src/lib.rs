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
//! ## Serde type mapping
//!
//! | Rust / serde type | XDR encoding |
//! |-------------------|--------------|
//! | `bool`            | 4-byte unsigned int: 0 (false) or 1 (true) |
//! | `i8`, `i16`, `i32` | 4-byte signed int (sign-extended) |
//! | `i64`             | 8-byte hyper integer |
//! | `u8`, `u16`, `u32` | 4-byte unsigned int (zero-extended) |
//! | `u64`             | 8-byte unsigned hyper integer |
//! | `f32`             | 4-byte IEEE 754 single-precision float |
//! | `f64`             | 8-byte IEEE 754 double-precision float |
//! | `char`            | 4-byte unsigned int (Unicode scalar) |
//! | `&str`, `String`  | 4-byte length + UTF-8 bytes + 0-3 zero-padding bytes |
//! | `&[u8]`, `Vec<u8>` | 4-byte length + bytes + 0-3 zero-padding bytes |
//! | `Option<T>`       | 4-byte bool discriminant + optional encoded T |
//! | `()` / unit struct | 0 bytes (XDR void) |
//! | Unit enum variant | 4-byte unsigned discriminant (variant index) |
//! | Newtype variant   | 4-byte discriminant + encoded inner value |
//! | Tuple/struct variant | 4-byte discriminant + fields consecutively |
//! | `Vec<T>` / seq    | 4-byte count + encoded elements |
//! | Tuple / tuple struct | fields encoded consecutively (no count prefix) |
//! | Struct            | fields encoded consecutively (no count prefix) |
//! | Map               | 4-byte count + alternating encoded keys and values |
//!
//! ## Example
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
//! let fh = FileHandle {
//!     inode: 0x0102030405060708,
//!     generation: 42,
//!     flags: 0,
//! };
//!
//! // Serialize to XDR bytes
//! let bytes = to_bytes(&fh).unwrap();
//! assert_eq!(bytes.len(), 16); // 8 + 4 + 4
//!
//! // Deserialize back
//! let decoded: FileHandle = from_bytes(&bytes).unwrap();
//! assert_eq!(fh, decoded);
//! ```

pub mod de;
pub mod error;
pub mod ser;

pub use de::{Deserializer, from_bytes, from_bytes_partial};
pub use error::{Error, Result};
pub use ser::{Serializer, to_bytes};

pub use serde::{Deserialize, Serialize};
