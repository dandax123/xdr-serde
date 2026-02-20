//! XDR Deserializer (RFC 4506)

use crate::error::{Error, Result};
use serde::de::{
    self, Deserialize, DeserializeOwned, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor,
};

/// Deserialize a value from XDR bytes. Returns the value and the number of bytes consumed.
pub fn from_bytes<T: DeserializeOwned>(input: &[u8]) -> Result<T> {
    let mut de = Deserializer::new(input);
    let value = T::deserialize(&mut de)?;
    Ok(value)
}

/// Deserialize a value from XDR bytes, also returning remaining unconsumed bytes.
pub fn from_bytes_partial<'de, T: Deserialize<'de>>(input: &'de [u8]) -> Result<(T, &'de [u8])> {
    let mut de = Deserializer::new(input);
    let value = T::deserialize(&mut de)?;
    Ok((value, de.remaining()))
}

/// The XDR deserializer. Reads from a byte slice, maintaining a cursor position.
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

    /// Consume exactly `n` bytes, returning a slice. Fails with UnexpectedEof.
    fn take(&mut self, n: usize) -> Result<&'de [u8]> {
        if self.pos + n > self.input.len() {
            return Err(Error::UnexpectedEof);
        }
        let slice = &self.input[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// Read a big-endian u32 (XDR basic block)
    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
    }

    /// Read a big-endian i32
    fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.take(4)?;
        Ok(i32::from_be_bytes(bytes.try_into().unwrap()))
    }

    /// Read a big-endian u64
    fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.take(8)?;
        Ok(u64::from_be_bytes(bytes.try_into().unwrap()))
    }

    /// Read a big-endian i64
    fn read_i64(&mut self) -> Result<i64> {
        let bytes = self.take(8)?;
        Ok(i64::from_be_bytes(bytes.try_into().unwrap()))
    }

    /// Read `n` bytes of data plus their 0–3 padding bytes.
    /// Returns a slice into the original input (zero-copy).
    fn read_padded_bytes(&mut self, n: usize) -> Result<&'de [u8]> {
        let data = self.take(n)?;
        let remainder = n % 4;
        if remainder != 0 {
            let padding_len = 4 - remainder;
            self.take(padding_len)?;
        }
        Ok(data)
    }

    /// Read a variable-length opaque or string:
    /// 4-byte length n, then n bytes + padding.
    fn read_variable_opaque(&mut self) -> Result<&'de [u8]> {
        let n = self.read_u32()? as usize;
        self.read_padded_bytes(n)
    }

    /// Read a variable-length opaque and copy it into an owned Vec.
    fn read_variable_opaque_owned(&mut self) -> Result<Vec<u8>> {
        Ok(self.read_variable_opaque()?.to_vec())
    }
}

// ── Main Deserializer impl ─────────────────────────────────────────────────

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::Unsupported(
            "deserialize_any (XDR is not self-describing)",
        ))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let v = self.read_u32()?;
        match v {
            0 => visitor.visit_bool(false),
            1 => visitor.visit_bool(true),
            _ => Err(Error::InvalidBool(v)),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i8(self.read_i32()? as i8)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i16(self.read_i32()? as i16)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i32(self.read_i32()?)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i64(self.read_i64()?)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u8(self.read_u32()? as u8)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u16(self.read_u32()? as u16)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u32(self.read_u32()?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u64(self.read_u64()?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let bytes = self.take(4)?;
        visitor.visit_f32(f32::from_be_bytes(bytes.try_into().unwrap()))
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let bytes = self.take(8)?;
        visitor.visit_f64(f64::from_be_bytes(bytes.try_into().unwrap()))
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let v = self.read_u32()?;
        let c = char::from_u32(v).ok_or(Error::InvalidString)?;
        visitor.visit_char(c)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let bytes = self.read_variable_opaque()?;
        let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidString)?;
        visitor.visit_str(s)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let bytes = self.read_variable_opaque_owned()?;
        let s = String::from_utf8(bytes).map_err(|_| Error::InvalidString)?;
        visitor.visit_string(s)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let bytes = self.read_variable_opaque()?;
        visitor.visit_bytes(bytes)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let bytes = self.read_variable_opaque_owned()?;
        visitor.visit_byte_buf(bytes)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let discriminant = self.read_u32()?;
        match discriminant {
            0 => visitor.visit_none(),
            1 => visitor.visit_some(self),
            v => Err(Error::InvalidOption(v)),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let count = self.read_u32()? as usize;
        visitor.visit_seq(SeqDeserializer::new(self, count))
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        // Fixed-length: no count prefix
        visitor.visit_seq(SeqDeserializer::new(self, len))
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_seq(SeqDeserializer::new(self, len))
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let count = self.read_u32()? as usize;
        visitor.visit_map(MapDeserializer::new(self, count))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        // XDR structure: fields in order, no count prefix
        visitor.visit_seq(SeqDeserializer::new(self, fields.len()))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_enum(EnumDeserializer::new(self))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        // Identifiers are discriminants in XDR context — read as u32
        visitor.visit_u32(self.read_u32()?)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::Unsupported(
            "deserialize_ignored_any (XDR is not self-describing)",
        ))
    }
}

// ── SeqDeserializer: fixed count ───────────────────────────────────────────

struct SeqDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
}

impl<'a, 'de> SeqDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, count: usize) -> Self {
        SeqDeserializer {
            de,
            remaining: count,
        }
    }
}

impl<'de, 'a> SeqAccess<'de> for SeqDeserializer<'a, 'de> {
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

// ── MapDeserializer ────────────────────────────────────────────────────────

struct MapDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
}

impl<'a, 'de> MapDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, count: usize) -> Self {
        MapDeserializer {
            de,
            remaining: count,
        }
    }
}

impl<'de, 'a> MapAccess<'de> for MapDeserializer<'a, 'de> {
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

// ── EnumDeserializer ───────────────────────────────────────────────────────

struct EnumDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> EnumDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        EnumDeserializer { de }
    }
}

impl<'de, 'a> EnumAccess<'de> for EnumDeserializer<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant)> {
        // Read the 4-byte discriminant as a u32
        let variant_index = self.de.read_u32()?;
        // Feed it to the seed as a u64 (serde's canonical discriminant type)
        let val = seed.deserialize(de::value::U32Deserializer::<Error>::new(variant_index))?;
        Ok((val, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for EnumDeserializer<'a, 'de> {
    type Error = Error;

    /// Unit variant — no data follows the discriminant
    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    /// Newtype variant — deserialize the inner value
    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(self.de)
    }

    /// Tuple variant — deserialize a fixed-length sequence of fields
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        visitor.visit_seq(SeqDeserializer::new(self.de, len))
    }

    /// Struct variant — deserialize fields consecutively by name list length
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_seq(SeqDeserializer::new(self.de, fields.len()))
    }
}
