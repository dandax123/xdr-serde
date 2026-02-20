//! XDR Serializer (RFC 4506)
//!
//! The [`Serializer`] is generic over any `W: std::io::Write`, enabling both
//! in-memory serialization (`to_bytes`) and streaming serialization (`to_writer`).
//!
//! ## Wire format summary
//! - All values are big-endian (network byte order)
//! - All items are padded to a multiple of 4 bytes
//! - Integers: 4 bytes (signed or unsigned), Hyper: 8 bytes
//! - Floats: IEEE 754, 4 bytes; Doubles: 8 bytes
//! - Strings/Bytes: 4-byte length prefix + data + 0–3 zero-padding bytes
//! - Sequences: 4-byte count prefix + elements
//! - Structs/Tuples: fields encoded consecutively, no length prefix
//! - Options: 4-byte bool discriminant (0=None, 1=Some) + optional value
//! - Enums (unit): 4-byte discriminant (variant index as u32)
//! - Enums (with data): 4-byte discriminant + encoded arm

use crate::error::{Error, Result};
use serde::ser::{self, Serialize};
use std::io::Write;

// ── Public entry points ────────────────────────────────────────────────────

/// Serialize `value` into a freshly allocated `Vec<u8>` of XDR bytes.
pub fn to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut ser = Serializer::new(Vec::new());
    value.serialize(&mut ser)?;
    Ok(ser.into_writer())
}

/// Serialize `value` as XDR bytes, writing directly into `writer`.
///
/// Unlike [`to_bytes`], this never allocates an intermediate buffer. Useful
/// when writing to a `TcpStream`, `File`, or any other `Write` sink.
pub fn to_writer<W: Write, T: Serialize>(mut writer: W, value: &T) -> Result<()> {
    let mut ser = Serializer::new(&mut writer);
    value.serialize(&mut ser)
}

// ── Serializer ─────────────────────────────────────────────────────────────

/// The XDR serializer. Generic over any `W: Write`.
///
/// Obtain one via [`to_bytes`] / [`to_writer`], or construct directly for
/// advanced use cases:
///
/// ```rust
/// use xdr_serde::ser::Serializer;
/// use serde::Serialize;
///
/// let mut buf = Vec::new();
/// let mut ser = Serializer::new(&mut buf);
/// 42u32.serialize(&mut ser).unwrap();
/// assert_eq!(buf, [0, 0, 0, 42]);
/// ```
pub struct Serializer<W: Write> {
    writer: W,
}

impl<W: Write> Serializer<W> {
    /// Create a new serializer that writes into `writer`.
    pub fn new(writer: W) -> Self {
        Serializer { writer }
    }

    /// Consume the serializer and return the inner writer.
    pub fn into_writer(self) -> W {
        self.writer
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer
            .write_all(bytes)
            .map_err(|e| Error::Io(e.to_string()))
    }

    fn write_u32(&mut self, v: u32) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    fn write_i32(&mut self, v: i32) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    fn write_u64(&mut self, v: u64) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    fn write_i64(&mut self, v: i64) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    /// Write `bytes` followed by enough zero bytes to reach a 4-byte boundary.
    pub(crate) fn write_padded_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.write_all(bytes)?;
        let remainder = bytes.len() % 4;
        if remainder != 0 {
            let pad = [0u8; 3];
            self.write_all(&pad[..4 - remainder])?;
        }
        Ok(())
    }

    /// XDR variable-length opaque: 4-byte length + padded data.
    fn write_opaque_variable(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > u32::MAX as usize {
            return Err(Error::LengthOverflow {
                max: u32::MAX,
                got: u32::MAX,
            });
        }
        self.write_u32(bytes.len() as u32)?;
        self.write_padded_bytes(bytes)
    }
}

// ── serde::Serializer impl ─────────────────────────────────────────────────

impl<'a, W: Write> ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    // ── Primitives ─────────────────────────────────────────────────────────

    /// XDR Bool → 4-byte unsigned int: 0 (false) or 1 (true)
    fn serialize_bool(self, v: bool) -> Result<()> {
        self.write_u32(if v { 1 } else { 0 })
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.write_i32(v as i32)
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.write_i32(v as i32)
    }
    /// XDR signed integer — 4 bytes, big-endian, two's complement
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.write_i32(v)
    }
    /// XDR hyper integer — 8 bytes, big-endian, two's complement
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.write_i64(v)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.write_u32(v as u32)
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.write_u32(v as u32)
    }
    /// XDR unsigned integer — 4 bytes, big-endian
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.write_u32(v)
    }
    /// XDR unsigned hyper integer — 8 bytes, big-endian
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.write_u64(v)
    }

    /// XDR single-precision float — IEEE 754, 4 bytes
    fn serialize_f32(self, v: f32) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }
    /// XDR double-precision float — IEEE 754, 8 bytes
    fn serialize_f64(self, v: f64) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    /// char → XDR unsigned int (Unicode scalar value, 4 bytes)
    fn serialize_char(self, v: char) -> Result<()> {
        self.write_u32(v as u32)
    }

    /// XDR string — 4-byte length + UTF-8 bytes + 0–3 zero-padding bytes
    fn serialize_str(self, v: &str) -> Result<()> {
        self.write_opaque_variable(v.as_bytes())
    }

    /// XDR variable-length opaque — 4-byte length + data + 0–3 padding bytes
    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.write_opaque_variable(v)
    }

    /// XDR optional-data void arm — 4-byte FALSE (0)
    fn serialize_none(self) -> Result<()> {
        self.write_u32(0)
    }

    /// XDR optional-data value arm — 4-byte TRUE (1) + encoded value
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        self.write_u32(1)?;
        value.serialize(self)
    }

    /// XDR void — 0 bytes
    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Ok(())
    }

    /// Unit enum variant → 4-byte unsigned discriminant (variant index)
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        self.write_u32(variant_index)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        if name == crate::FIXED_OPAQUE_TOKEN {
            // The value is a `FixedOpaqueHelper(&[u8])` which will call
            // `serialize_bytes` — intercept that through a delegate serializer
            // that writes padded bytes without a length prefix.
            value.serialize(FixedOpaqueSerializer(self))
        } else {
            value.serialize(self)
        }
    }

    /// Enum newtype variant → 4-byte discriminant + encoded inner value
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()> {
        self.write_u32(variant_index)?;
        value.serialize(self)
    }

    /// XDR variable-length array → 4-byte element count + elements
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let l = len.ok_or(Error::LengthRequired)?;
        self.write_u32(l as u32)?;
        Ok(self)
    }

    /// XDR fixed-length array / structure — elements without a length prefix
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(self)
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(self)
    }

    /// Enum tuple variant → 4-byte discriminant + fields (no inner length prefix)
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.write_u32(variant_index)?;
        Ok(self)
    }

    /// Map → 4-byte pair count + alternating key/value pairs
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let l = len.ok_or(Error::LengthRequired)?;
        self.write_u32(l as u32)?;
        Ok(self)
    }

    /// XDR structure — fields encoded consecutively, no count prefix
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    /// Enum struct variant → 4-byte discriminant + fields consecutively
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.write_u32(variant_index)?;
        Ok(self)
    }
}

// ── Compound serializer impls ──────────────────────────────────────────────

macro_rules! forward_serialize_element {
    ($t:ty) => {
        impl<'a, W: Write> $t for &'a mut Serializer<W> {
            type Ok = ();
            type Error = Error;
            fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
                value.serialize(&mut **self)
            }
            fn end(self) -> Result<()> {
                Ok(())
            }
        }
    };
}

macro_rules! forward_serialize_field {
    ($t:ty) => {
        impl<'a, W: Write> $t for &'a mut Serializer<W> {
            type Ok = ();
            type Error = Error;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                _key: &'static str,
                value: &T,
            ) -> Result<()> {
                value.serialize(&mut **self)
            }
            fn end(self) -> Result<()> {
                Ok(())
            }
        }
    };
}

forward_serialize_element!(ser::SerializeSeq);
forward_serialize_element!(ser::SerializeTuple);

impl<'a, W: Write> ser::SerializeTupleStruct for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeTupleVariant for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeMap for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        key.serialize(&mut **self)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}

forward_serialize_field!(ser::SerializeStruct);
forward_serialize_field!(ser::SerializeStructVariant);

// ── FixedOpaqueSerializer ──────────────────────────────────────────────────
//
// A thin delegating serializer used only to intercept the `serialize_bytes`
// call that `FixedOpaqueHelper` makes, and route it through `write_padded_bytes`
// (no length prefix) instead of the normal `write_opaque_variable` (with prefix).

struct FixedOpaqueSerializer<'a, W: Write>(&'a mut Serializer<W>);

impl<'a, W: Write> ser::Serializer for FixedOpaqueSerializer<'a, W> {
    type Ok = ();
    type Error = Error;

    // The only method we actually use:
    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.0.write_padded_bytes(v)
    }

    // Everything else delegates unchanged — in practice these are never called
    // via the fixed_opaque path, but we must implement the full trait.
    type SerializeSeq = <&'a mut Serializer<W> as ser::Serializer>::SerializeSeq;
    type SerializeTuple = <&'a mut Serializer<W> as ser::Serializer>::SerializeTuple;
    type SerializeTupleStruct = <&'a mut Serializer<W> as ser::Serializer>::SerializeTupleStruct;
    type SerializeTupleVariant = <&'a mut Serializer<W> as ser::Serializer>::SerializeTupleVariant;
    type SerializeMap = <&'a mut Serializer<W> as ser::Serializer>::SerializeMap;
    type SerializeStruct = <&'a mut Serializer<W> as ser::Serializer>::SerializeStruct;
    type SerializeStructVariant =
        <&'a mut Serializer<W> as ser::Serializer>::SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.0.serialize_bool(v)
    }
    fn serialize_i8(self, v: i8) -> Result<()> {
        ser::Serializer::serialize_i8(self.0, v)
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        ser::Serializer::serialize_i16(self.0, v)
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        ser::Serializer::serialize_i32(self.0, v)
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        ser::Serializer::serialize_i64(self.0, v)
    }
    fn serialize_u8(self, v: u8) -> Result<()> {
        ser::Serializer::serialize_u8(self.0, v)
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        ser::Serializer::serialize_u16(self.0, v)
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        ser::Serializer::serialize_u32(self.0, v)
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        ser::Serializer::serialize_u64(self.0, v)
    }
    fn serialize_f32(self, v: f32) -> Result<()> {
        ser::Serializer::serialize_f32(self.0, v)
    }
    fn serialize_f64(self, v: f64) -> Result<()> {
        ser::Serializer::serialize_f64(self.0, v)
    }
    fn serialize_char(self, v: char) -> Result<()> {
        ser::Serializer::serialize_char(self.0, v)
    }
    fn serialize_str(self, v: &str) -> Result<()> {
        ser::Serializer::serialize_str(self.0, v)
    }
    fn serialize_none(self) -> Result<()> {
        ser::Serializer::serialize_none(self.0)
    }
    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }
    fn serialize_unit_struct(self, n: &'static str) -> Result<()> {
        ser::Serializer::serialize_unit_struct(self.0, n)
    }
    fn serialize_unit_variant(self, n: &'static str, idx: u32, v: &'static str) -> Result<()> {
        ser::Serializer::serialize_unit_variant(self.0, n, idx, v)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        ser::Serializer::serialize_some(self.0, value)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, n: &'static str, v: &T) -> Result<()> {
        ser::Serializer::serialize_newtype_struct(self.0, n, v)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        n: &'static str,
        idx: u32,
        var: &'static str,
        v: &T,
    ) -> Result<()> {
        ser::Serializer::serialize_newtype_variant(self.0, n, idx, var, v)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        ser::Serializer::serialize_seq(self.0, len)
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        ser::Serializer::serialize_tuple(self.0, len)
    }
    fn serialize_tuple_struct(
        self,
        n: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        ser::Serializer::serialize_tuple_struct(self.0, n, len)
    }
    fn serialize_tuple_variant(
        self,
        n: &'static str,
        idx: u32,
        var: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        ser::Serializer::serialize_tuple_variant(self.0, n, idx, var, len)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        ser::Serializer::serialize_map(self.0, len)
    }
    fn serialize_struct(self, n: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        ser::Serializer::serialize_struct(self.0, n, len)
    }
    fn serialize_struct_variant(
        self,
        n: &'static str,
        idx: u32,
        var: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        ser::Serializer::serialize_struct_variant(self.0, n, idx, var, len)
    }
}
