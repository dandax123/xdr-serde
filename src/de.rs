//! XDR Deserializer (RFC 4506)
//!
//! Two deserializers are provided:
//!
//! - [`Deserializer`] — zero-copy slice-based deserializer. Strings and byte slices borrow
//!   directly from the input buffer when possible.
//! - [`ReaderDeserializer`] — `io::Read`-based deserializer. Uses `read_exact` internally;
//!   all string/byte outputs are owned. Use this when reading from a socket, file, etc.

use crate::error::{Error, Result};
use serde::de::{
    self, Deserialize, DeserializeOwned, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor,
};
use std::io::Read;

// ── Slice-based entry points ───────────────────────────────────────────────

/// Deserialize a value from a complete XDR byte slice.
pub fn from_bytes<T: DeserializeOwned>(input: &[u8]) -> Result<T> {
    let mut de = Deserializer::new(input);
    T::deserialize(&mut de)
}

/// Deserialize a value from a byte slice, also returning any unconsumed bytes.
///
/// ```rust
/// use xdr_serde::{to_bytes, from_bytes_partial};
/// let mut buf = to_bytes(&1u32).unwrap();
/// buf.extend(to_bytes(&2u32).unwrap());
/// let (a, rest) = from_bytes_partial::<u32>(&buf).unwrap();
/// let (b, _)    = from_bytes_partial::<u32>(rest).unwrap();
/// assert_eq!((a, b), (1, 2));
/// ```
pub fn from_bytes_partial<'de, T: Deserialize<'de>>(input: &'de [u8]) -> Result<(T, &'de [u8])> {
    let mut de = Deserializer::new(input);
    let value = T::deserialize(&mut de)?;
    Ok((value, de.remaining()))
}

// ── Reader-based entry point ───────────────────────────────────────────────

/// Deserialize a value from anything that implements [`std::io::Read`].
///
/// Only the bytes necessary to decode `T` are consumed from `reader`.
/// Wrap the reader in a [`std::io::BufReader`] for better performance over
/// sockets or files.
pub fn from_reader<R: Read, T: DeserializeOwned>(reader: R) -> Result<T> {
    let mut de = ReaderDeserializer::new(reader);
    T::deserialize(&mut de)
}

// ══════════════════════════════════════════════════════════════════════════
// Slice-based Deserializer
// ══════════════════════════════════════════════════════════════════════════

/// Zero-copy XDR deserializer backed by a byte slice.
pub struct Deserializer<'de> {
    input: &'de [u8],
    pos: usize,
}

impl<'de> Deserializer<'de> {
    pub fn new(input: &'de [u8]) -> Self {
        Deserializer { input, pos: 0 }
    }

    /// Returns the unconsumed portion of the input buffer.
    pub fn remaining(&self) -> &'de [u8] {
        &self.input[self.pos..]
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'de [u8]> {
        if self.pos + n > self.input.len() {
            return Err(Error::UnexpectedEof);
        }
        let slice = &self.input[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(crate) fn read_i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(crate) fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub(crate) fn read_i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    /// Read `n` data bytes + 0–3 zero-padding bytes to reach a 4-byte boundary.
    /// Returns a zero-copy slice of exactly `n` bytes.
    pub(crate) fn read_padded_bytes(&mut self, n: usize) -> Result<&'de [u8]> {
        let data = self.take(n)?;
        let remainder = n % 4;
        if remainder != 0 {
            self.take(4 - remainder)?;
        }
        Ok(data)
    }

    /// Variable-length opaque: read 4-byte length then `n` padded bytes.
    fn read_variable_opaque(&mut self) -> Result<&'de [u8]> {
        let n = self.read_u32()? as usize;
        self.read_padded_bytes(n)
    }

    fn read_variable_opaque_owned(&mut self) -> Result<Vec<u8>> {
        Ok(self.read_variable_opaque()?.to_vec())
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _v: V) -> Result<V::Value> {
        Err(Error::Unsupported(
            "deserialize_any (XDR is not self-describing)",
        ))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.read_u32()? {
            0 => visitor.visit_bool(false),
            1 => visitor.visit_bool(true),
            v => Err(Error::InvalidBool(v)),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i8(self.read_i32()? as i8)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i16(self.read_i32()? as i16)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i32(self.read_i32()?)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i64(self.read_i64()?)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u8(self.read_u32()? as u8)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u16(self.read_u32()? as u16)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u32(self.read_u32()?)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u64(self.read_u64()?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_f32(f32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_f64(f64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn deserialize_char<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_char(char::from_u32(self.read_u32()?).ok_or(Error::InvalidString)?)
    }

    fn deserialize_str<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let bytes = self.read_variable_opaque()?;
        v.visit_str(std::str::from_utf8(bytes).map_err(|_| Error::InvalidString)?)
    }
    fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let s = String::from_utf8(self.read_variable_opaque_owned()?)
            .map_err(|_| Error::InvalidString)?;
        v.visit_string(s)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_bytes(self.read_variable_opaque()?)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_byte_buf(self.read_variable_opaque_owned()?)
    }

    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        match self.read_u32()? {
            0 => v.visit_none(),
            1 => v.visit_some(self),
            n => Err(Error::InvalidOption(n)),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, _: &'static str, v: V) -> Result<V::Value> {
        v.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        v: V,
    ) -> Result<V::Value> {
        if name == crate::FIXED_OPAQUE_TOKEN {
            // Pass a special deserializer whose deserialize_tuple reads N raw
            // padded bytes (no per-element XDR padding). The visitor will call
            // T::deserialize(inner_de), which for [u8; N] calls deserialize_tuple(N, ...).
            v.visit_newtype_struct(FixedOpaqueSliceDe(self))
        } else {
            v.visit_newtype_struct(self)
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let count = self.read_u32()? as usize;
        v.visit_seq(SliceSeqAccess::new(self, count))
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value> {
        v.visit_seq(SliceSeqAccess::new(self, len))
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        v: V,
    ) -> Result<V::Value> {
        v.visit_seq(SliceSeqAccess::new(self, len))
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        v.visit_seq(SliceSeqAccess::new(self, fields.len()))
    }
    fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let count = self.read_u32()? as usize;
        v.visit_map(SliceMapAccess::new(self, count))
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        v.visit_enum(SliceEnumAccess::new(self))
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u32(self.read_u32()?)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, _v: V) -> Result<V::Value> {
        Err(Error::Unsupported(
            "deserialize_ignored_any (XDR is not self-describing)",
        ))
    }
}

// ── FixedOpaqueSliceDe: inner deserializer for fixed-length opaque ─────────
//
// Passed to the FixedOpaqueVisitor's visit_newtype_struct. When the visitor
// calls T::deserialize(this_de) for T=[u8;N], serde's derived impl calls
// this_de.deserialize_tuple(N, array_visitor). We intercept that, read N bytes
// + padding in one shot, and serve them as raw u8 bytes without per-element
// 4-byte XDR promotion.

struct FixedOpaqueSliceDe<'a, 'de: 'a>(&'a mut Deserializer<'de>);

impl<'de, 'a> de::Deserializer<'de> for FixedOpaqueSliceDe<'a, 'de> {
    type Error = Error;

    /// Read N raw bytes + padding; yield them as a seq of u8 values.
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        let bytes = self.0.read_padded_bytes(len)?;
        visitor.visit_seq(RawByteSeqAccess {
            data: bytes,
            pos: 0,
        })
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        let bytes = self.0.read_padded_bytes(len)?;
        visitor.visit_seq(RawByteSeqAccess {
            data: bytes,
            pos: 0,
        })
    }

    // Fallback: let the inner de handle everything else unchanged.
    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        self.0.deserialize_any(v)
    }
    fn deserialize_bool<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_bool(self.0, v)
    }
    fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i8(self.0, v)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i16(self.0, v)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i32(self.0, v)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i64(self.0, v)
    }
    fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u8(self.0, v)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u16(self.0, v)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u32(self.0, v)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u64(self.0, v)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_f32(self.0, v)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_f64(self.0, v)
    }
    fn deserialize_char<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_char(self.0, v)
    }
    fn deserialize_str<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_str(self.0, v)
    }
    fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_string(self.0, v)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_bytes(self.0, v)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_byte_buf(self.0, v)
    }
    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_option(self.0, v)
    }
    fn deserialize_unit<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_unit(self.0, v)
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, n: &'static str, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_unit_struct(self.0, n, v)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        n: &'static str,
        v: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_newtype_struct(self.0, n, v)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_seq(self.0, v)
    }
    fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_map(self.0, v)
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        n: &'static str,
        f: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_struct(self.0, n, f, v)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        n: &'static str,
        vars: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_enum(self.0, n, vars, v)
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_identifier(self.0, v)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_ignored_any(self.0, v)
    }
}

// A SeqAccess that yields raw u8 bytes from a slice without any XDR padding.
struct RawByteSeqAccess<'de> {
    data: &'de [u8],
    pos: usize,
}
impl<'de> SeqAccess<'de> for RawByteSeqAccess<'de> {
    type Error = Error;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if self.pos >= self.data.len() {
            return Ok(None);
        }
        let byte = self.data[self.pos];
        self.pos += 1;
        seed.deserialize(de::value::U8Deserializer::new(byte))
            .map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.data.len() - self.pos)
    }
}

// ── Slice-based compound access ────────────────────────────────────────────

struct SliceSeqAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
}
impl<'a, 'de> SliceSeqAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, count: usize) -> Self {
        Self {
            de,
            remaining: count,
        }
    }
}
impl<'de, 'a> SeqAccess<'de> for SliceSeqAccess<'a, 'de> {
    type Error = Error;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

struct SliceMapAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
}
impl<'a, 'de> SliceMapAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, count: usize) -> Self {
        Self {
            de,
            remaining: count,
        }
    }
}
impl<'de, 'a> MapAccess<'de> for SliceMapAccess<'a, 'de> {
    type Error = Error;
    fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(&mut *self.de)
    }
}

struct SliceEnumAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}
impl<'a, 'de> SliceEnumAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        Self { de }
    }
}
impl<'de, 'a> EnumAccess<'de> for SliceEnumAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<V: de::DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let idx = self.de.read_u32()?;
        let val = seed.deserialize(de::value::U32Deserializer::<crate::error::Error>::new(idx))?;
        Ok((val, self))
    }
}
impl<'de, 'a> VariantAccess<'de> for SliceEnumAccess<'a, 'de> {
    type Error = Error;
    fn unit_variant(self) -> Result<()> {
        Ok(())
    }
    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(self.de)
    }
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value> {
        v.visit_seq(SliceSeqAccess::new(self.de, len))
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        v.visit_seq(SliceSeqAccess::new(self.de, fields.len()))
    }
}

// ══════════════════════════════════════════════════════════════════════════
// Reader-based Deserializer
// ══════════════════════════════════════════════════════════════════════════

/// XDR deserializer backed by any [`std::io::Read`] source.
///
/// All decoded strings and byte sequences are returned as owned values.
pub struct ReaderDeserializer<R: Read> {
    reader: R,
}

impl<R: Read> ReaderDeserializer<R> {
    pub fn new(reader: R) -> Self {
        ReaderDeserializer { reader }
    }

    pub fn into_reader(self) -> R {
        self.reader
    }

    pub(crate) fn read_exact_buf(&mut self, n: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; n];
        self.reader.read_exact(&mut buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Error::UnexpectedEof
            } else {
                Error::Io(e.to_string())
            }
        })?;
        Ok(buf)
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32> {
        let b = self.read_exact_buf(4)?;
        Ok(u32::from_be_bytes(b.try_into().unwrap()))
    }

    pub(crate) fn read_i32(&mut self) -> Result<i32> {
        let b = self.read_exact_buf(4)?;
        Ok(i32::from_be_bytes(b.try_into().unwrap()))
    }

    pub(crate) fn read_u64(&mut self) -> Result<u64> {
        let b = self.read_exact_buf(8)?;
        Ok(u64::from_be_bytes(b.try_into().unwrap()))
    }

    pub(crate) fn read_i64(&mut self) -> Result<i64> {
        let b = self.read_exact_buf(8)?;
        Ok(i64::from_be_bytes(b.try_into().unwrap()))
    }

    pub(crate) fn read_padded_bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        let data = self.read_exact_buf(n)?;
        let remainder = n % 4;
        if remainder != 0 {
            self.read_exact_buf(4 - remainder)?;
        }
        Ok(data)
    }

    fn read_variable_opaque(&mut self) -> Result<Vec<u8>> {
        let n = self.read_u32()? as usize;
        self.read_padded_bytes(n)
    }
}

impl<'de, R: Read> de::Deserializer<'de> for &mut ReaderDeserializer<R> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _v: V) -> Result<V::Value> {
        Err(Error::Unsupported(
            "deserialize_any (XDR is not self-describing)",
        ))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.read_u32()? {
            0 => visitor.visit_bool(false),
            1 => visitor.visit_bool(true),
            v => Err(Error::InvalidBool(v)),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i8(self.read_i32()? as i8)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i16(self.read_i32()? as i16)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i32(self.read_i32()?)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_i64(self.read_i64()?)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u8(self.read_u32()? as u8)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u16(self.read_u32()? as u16)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u32(self.read_u32()?)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u64(self.read_u64()?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let b = self.read_exact_buf(4)?;
        v.visit_f32(f32::from_be_bytes(b.try_into().unwrap()))
    }
    fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let b = self.read_exact_buf(8)?;
        v.visit_f64(f64::from_be_bytes(b.try_into().unwrap()))
    }

    fn deserialize_char<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_char(char::from_u32(self.read_u32()?).ok_or(Error::InvalidString)?)
    }

    fn deserialize_str<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        self.deserialize_string(v)
    }
    fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let s =
            String::from_utf8(self.read_variable_opaque()?).map_err(|_| Error::InvalidString)?;
        v.visit_string(s)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        self.deserialize_byte_buf(v)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_byte_buf(self.read_variable_opaque()?)
    }

    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        match self.read_u32()? {
            0 => v.visit_none(),
            1 => v.visit_some(self),
            n => Err(Error::InvalidOption(n)),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, _: &'static str, v: V) -> Result<V::Value> {
        v.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        v: V,
    ) -> Result<V::Value> {
        if name == crate::FIXED_OPAQUE_TOKEN {
            v.visit_newtype_struct(FixedOpaqueReaderDe(self))
        } else {
            v.visit_newtype_struct(self)
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let count = self.read_u32()? as usize;
        v.visit_seq(ReaderSeqAccess::new(self, count))
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value> {
        v.visit_seq(ReaderSeqAccess::new(self, len))
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        v: V,
    ) -> Result<V::Value> {
        v.visit_seq(ReaderSeqAccess::new(self, len))
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        v.visit_seq(ReaderSeqAccess::new(self, fields.len()))
    }
    fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let count = self.read_u32()? as usize;
        v.visit_map(ReaderMapAccess::new(self, count))
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        v.visit_enum(ReaderEnumAccess::new(self))
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u32(self.read_u32()?)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, _v: V) -> Result<V::Value> {
        Err(Error::Unsupported(
            "deserialize_ignored_any (XDR is not self-describing)",
        ))
    }
}

// ── FixedOpaqueReaderDe: reader counterpart of FixedOpaqueSliceDe ──────────

struct FixedOpaqueReaderDe<'a, R: Read>(&'a mut ReaderDeserializer<R>);

impl<'de, 'a, R: Read> de::Deserializer<'de> for FixedOpaqueReaderDe<'a, R> {
    type Error = Error;

    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        let bytes = self.0.read_padded_bytes(len)?;
        visitor.visit_seq(OwnedByteSeqAccess {
            data: bytes,
            pos: 0,
        })
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        v: V,
    ) -> Result<V::Value> {
        let bytes = self.0.read_padded_bytes(len)?;
        v.visit_seq(OwnedByteSeqAccess {
            data: bytes,
            pos: 0,
        })
    }

    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_any(self.0, v)
    }
    fn deserialize_bool<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_bool(self.0, v)
    }
    fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i8(self.0, v)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i16(self.0, v)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i32(self.0, v)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_i64(self.0, v)
    }
    fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u8(self.0, v)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u16(self.0, v)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u32(self.0, v)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_u64(self.0, v)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_f32(self.0, v)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_f64(self.0, v)
    }
    fn deserialize_char<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_char(self.0, v)
    }
    fn deserialize_str<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_str(self.0, v)
    }
    fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_string(self.0, v)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_bytes(self.0, v)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_byte_buf(self.0, v)
    }
    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_option(self.0, v)
    }
    fn deserialize_unit<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_unit(self.0, v)
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, n: &'static str, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_unit_struct(self.0, n, v)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        n: &'static str,
        v: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_newtype_struct(self.0, n, v)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_seq(self.0, v)
    }
    fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_map(self.0, v)
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        n: &'static str,
        f: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_struct(self.0, n, f, v)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        n: &'static str,
        vars: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_enum(self.0, n, vars, v)
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_identifier(self.0, v)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        de::Deserializer::deserialize_ignored_any(self.0, v)
    }
}

struct OwnedByteSeqAccess {
    data: Vec<u8>,
    pos: usize,
}
impl<'de> SeqAccess<'de> for OwnedByteSeqAccess {
    type Error = Error;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if self.pos >= self.data.len() {
            return Ok(None);
        }
        let byte = self.data[self.pos];
        self.pos += 1;
        seed.deserialize(de::value::U8Deserializer::new(byte))
            .map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.data.len() - self.pos)
    }
}

// ── Reader-based compound access ───────────────────────────────────────────

struct ReaderSeqAccess<'a, R: Read> {
    de: &'a mut ReaderDeserializer<R>,
    remaining: usize,
}
impl<'a, R: Read> ReaderSeqAccess<'a, R> {
    fn new(de: &'a mut ReaderDeserializer<R>, count: usize) -> Self {
        Self {
            de,
            remaining: count,
        }
    }
}
impl<'de, 'a, R: Read> SeqAccess<'de> for ReaderSeqAccess<'a, R> {
    type Error = Error;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

struct ReaderMapAccess<'a, R: Read> {
    de: &'a mut ReaderDeserializer<R>,
    remaining: usize,
}
impl<'a, R: Read> ReaderMapAccess<'a, R> {
    fn new(de: &'a mut ReaderDeserializer<R>, count: usize) -> Self {
        Self {
            de,
            remaining: count,
        }
    }
}
impl<'de, 'a, R: Read> MapAccess<'de> for ReaderMapAccess<'a, R> {
    type Error = Error;
    fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(&mut *self.de)
    }
}

struct ReaderEnumAccess<'a, R: Read> {
    de: &'a mut ReaderDeserializer<R>,
}
impl<'a, R: Read> ReaderEnumAccess<'a, R> {
    fn new(de: &'a mut ReaderDeserializer<R>) -> Self {
        Self { de }
    }
}
impl<'de, 'a, R: Read> EnumAccess<'de> for ReaderEnumAccess<'a, R> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<V: de::DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let idx = self.de.read_u32()?;
        let val = seed.deserialize(de::value::U32Deserializer::<crate::error::Error>::new(idx))?;
        Ok((val, self))
    }
}
impl<'de, 'a, R: Read> VariantAccess<'de> for ReaderEnumAccess<'a, R> {
    type Error = Error;
    fn unit_variant(self) -> Result<()> {
        Ok(())
    }
    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(self.de)
    }
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value> {
        v.visit_seq(ReaderSeqAccess::new(self.de, len))
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value> {
        v.visit_seq(ReaderSeqAccess::new(self.de, fields.len()))
    }
}
