//! XDR Serializer (RFC 4506)
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

/// Serialize a value into XDR bytes.
pub fn to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut ser = Serializer::new();
    value.serialize(&mut ser)?;
    Ok(ser.output)
}

/// The XDR serializer. Writes encoded bytes into an internal buffer.
pub struct Serializer {
    pub(crate) output: Vec<u8>,
}

impl Serializer {
    pub fn new() -> Self {
        Serializer { output: Vec::new() }
    }

    /// Write a big-endian u32
    fn write_u32(&mut self, v: u32) {
        self.output.extend_from_slice(&v.to_be_bytes());
    }

    /// Write a big-endian i32
    fn write_i32(&mut self, v: i32) {
        self.output.extend_from_slice(&v.to_be_bytes());
    }

    /// Write a big-endian u64
    fn write_u64(&mut self, v: u64) {
        self.output.extend_from_slice(&v.to_be_bytes());
    }

    /// Write a big-endian i64
    fn write_i64(&mut self, v: i64) {
        self.output.extend_from_slice(&v.to_be_bytes());
    }

    /// Write bytes followed by zero-padding to reach a 4-byte boundary
    fn write_padded_bytes(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
        let remainder = bytes.len() % 4;
        if remainder != 0 {
            let padding = 4 - remainder;
            for _ in 0..padding {
                self.output.push(0u8);
            }
        }
    }

    /// Encode a variable-length byte buffer: 4-byte length + padded data
    fn write_opaque_variable(&mut self, bytes: &[u8]) -> Result<()> {
        let len = bytes.len();
        if len > u32::MAX as usize {
            return Err(Error::LengthOverflow {
                max: u32::MAX,
                got: u32::MAX,
            });
        }
        self.write_u32(len as u32);
        self.write_padded_bytes(bytes);
        Ok(())
    }
}

impl Default for Serializer {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    // Compound types
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    // ── Primitive types ────────────────────────────────────────────────────

    /// XDR Bool: encoded as unsigned int 0 (false) or 1 (true) — 4 bytes
    fn serialize_bool(self, v: bool) -> Result<()> {
        self.write_u32(if v { 1 } else { 0 });
        Ok(())
    }

    /// Narrow integers promoted to XDR int (4 bytes signed)
    fn serialize_i8(self, v: i8) -> Result<()> {
        self.write_i32(v as i32);
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.write_i32(v as i32);
        Ok(())
    }

    /// XDR signed integer — 4 bytes, big-endian, two's complement
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.write_i32(v);
        Ok(())
    }

    /// XDR hyper integer — 8 bytes, big-endian, two's complement
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.write_i64(v);
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.write_u32(v as u32);
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.write_u32(v as u32);
        Ok(())
    }

    /// XDR unsigned integer — 4 bytes, big-endian
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.write_u32(v);
        Ok(())
    }

    /// XDR unsigned hyper integer — 8 bytes, big-endian
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.write_u64(v);
        Ok(())
    }

    /// XDR single-precision float — IEEE 754, 4 bytes, big-endian
    fn serialize_f32(self, v: f32) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    /// XDR double-precision float — IEEE 754, 8 bytes, big-endian
    fn serialize_f64(self, v: f64) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    /// XDR char — encoded as XDR unsigned int (4 bytes)
    fn serialize_char(self, v: char) -> Result<()> {
        self.write_u32(v as u32);
        Ok(())
    }

    /// XDR string — 4-byte length prefix + UTF-8 bytes + 0–3 zero-padding bytes
    fn serialize_str(self, v: &str) -> Result<()> {
        self.write_opaque_variable(v.as_bytes())
    }

    /// XDR variable-length opaque — 4-byte length + data + 0–3 padding bytes
    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.write_opaque_variable(v)
    }

    /// XDR optional-data (void arm) — encoded as bool 0 (FALSE)
    fn serialize_none(self) -> Result<()> {
        self.write_u32(0);
        Ok(())
    }

    /// XDR optional-data (value arm) — encoded as bool 1 (TRUE) + value
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        self.write_u32(1);
        value.serialize(self)
    }

    /// XDR void — zero bytes
    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    /// Unit struct — zero bytes (no name encoding in XDR)
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Ok(())
    }

    /// Unit enum variant — encoded as its variant index as XDR unsigned int
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        self.write_u32(variant_index);
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()> {
        value.serialize(self)
    }

    /// Enum variant with one value — 4-byte discriminant + encoded value
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()> {
        self.write_u32(variant_index);
        value.serialize(self)
    }

    /// XDR variable-length array — 4-byte element count + elements
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        match len {
            Some(l) => {
                self.write_u32(l as u32);
                Ok(self)
            }
            None => Err(Error::LengthRequired),
        }
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

    /// Tuple enum variant — 4-byte discriminant + fields (no inner length)
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.write_u32(variant_index);
        Ok(self)
    }

    /// Map — serialized as a variable-length array of (key, value) pairs
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        match len {
            Some(l) => {
                self.write_u32(l as u32);
                Ok(self)
            }
            None => Err(Error::LengthRequired),
        }
    }

    /// XDR structure — fields encoded consecutively without a count prefix
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    /// Struct enum variant — 4-byte discriminant + fields consecutively
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.write_u32(variant_index);
        Ok(self)
    }
}

// ── Compound serializer impls ──────────────────────────────────────────────

impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeMap for &'a mut Serializer {
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

impl<'a> ser::SerializeStruct for &'a mut Serializer {
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

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
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
