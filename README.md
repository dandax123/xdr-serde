# xdr-serde

A pure-Rust implementation of [XDR (eXternal Data Representation, RFC 4506)](https://www.rfc-editor.org/rfc/rfc4506) serialization and deserialization, built on top of the [`serde`](https://serde.rs) framework.

XDR is the binary wire encoding used by ONC RPC protocols — most notably **NFS** (Network File System). It encodes all values in big-endian (network) byte order, and pads every item to a multiple of 4 bytes.

---

## Features

- Full RFC 4506 compliance — integers, hyper integers, floats, strings, opaque data, fixed and variable-length arrays, structures, discriminated unions, optional-data, and void
- Zero-copy deserialization via borrowed `&'de [u8]` slices
- Partial deserialization with `from_bytes_partial` — consume only what you need from a packet and get the remaining bytes back
- Clean, descriptive error types for every failure mode
- No `unsafe` code
- No dependencies beyond `serde` itself

---

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
xdr-serde = "0.1"
serde = { version = "1", features = ["derive"] }
```

---

## Quick start

```rust
use serde::{Deserialize, Serialize};
use xdr_serde::{from_bytes, to_bytes};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FileHandle {
    inode: u64,
    generation: u32,
    flags: u32,
}

let fh = FileHandle {
    inode: 0x0102030405060708,
    generation: 42,
    flags: 0,
};

// Serialize → XDR bytes (big-endian, 4-byte aligned)
let bytes = to_bytes(&fh).unwrap();
assert_eq!(bytes.len(), 16); // 8 (u64) + 4 (u32) + 4 (u32)

// Deserialize back
let decoded: FileHandle = from_bytes(&bytes).unwrap();
assert_eq!(fh, decoded);
```

---

## API

### Serialization

```rust
// Serialize any serde::Serialize type to a Vec
pub fn to_bytes(value: &T) -> Result<Vec>;
```

### Deserialization

```rust
// Deserialize from a byte slice, consuming the entire input
pub fn from_bytes(input: &[u8]) -> Result;

// Deserialize and return the remaining unconsumed bytes — useful for streaming protocols
pub fn from_bytes_partial>(input: &'de [u8]) -> Result;
```

### Partial deserialization example

```rust
// Two back-to-back XDR values in one buffer
let mut buf = to_bytes(&42u32).unwrap();
buf.extend(to_bytes(&"hello".to_string()).unwrap());

let (first, rest) = from_bytes_partial::(&buf).unwrap();
assert_eq!(first, 42);

let (second, _) = from_bytes_partial::(rest).unwrap();
assert_eq!(second, "hello");
```

---

## Type mapping

The table below shows how every Rust/serde type maps to the XDR wire format defined in RFC 4506.

| Rust / serde type         | XDR type                       | Wire encoding                                                        |
| ------------------------- | ------------------------------ | -------------------------------------------------------------------- |
| `bool`                    | Boolean                        | 4-byte unsigned int: `0` (false) or `1` (true)                       |
| `i8`, `i16`, `i32`        | Integer                        | 4 bytes, big-endian, two's complement (sign-extended)                |
| `i64`                     | Hyper Integer                  | 8 bytes, big-endian, two's complement                                |
| `u8`, `u16`, `u32`        | Unsigned Integer               | 4 bytes, big-endian (zero-extended)                                  |
| `u64`                     | Unsigned Hyper Integer         | 8 bytes, big-endian                                                  |
| `f32`                     | Float                          | 4 bytes, IEEE 754 single-precision, big-endian                       |
| `f64`                     | Double                         | 8 bytes, IEEE 754 double-precision, big-endian                       |
| `char`                    | Unsigned Integer               | 4-byte Unicode scalar value                                          |
| `&str`, `String`          | String                         | 4-byte length `n` + `n` UTF-8 bytes + 0–3 zero-padding bytes         |
| `&[u8]`, `Vec<u8>`        | Variable-length Opaque         | 4-byte length `n` + `n` bytes + 0–3 zero-padding bytes               |
| `Option<T>`               | Optional-data                  | 4-byte bool discriminant (`0`=None, `1`=Some) + optional encoded `T` |
| `()`, unit struct         | Void                           | 0 bytes                                                              |
| Unit enum variant         | Enumeration                    | 4-byte unsigned int (serde variant index)                            |
| Newtype enum variant      | Discriminated Union            | 4-byte discriminant + encoded inner value                            |
| Tuple/struct enum variant | Discriminated Union            | 4-byte discriminant + fields consecutively                           |
| `Vec<T>`, seq             | Variable-length Array          | 4-byte element count + encoded elements                              |
| Tuple, tuple struct       | Fixed-length Array / Structure | Fields encoded consecutively, no length prefix                       |
| Struct                    | Structure                      | Fields encoded consecutively, no length prefix                       |
| Map                       | Variable-length Array          | 4-byte pair count + alternating encoded key/value pairs              |

> **Note:** XDR is not self-describing. The deserializer must know the target type at compile time, just as with formats like `bincode`. `deserialize_any` is not supported.

---

## Wire format details

### 4-byte alignment (§3)

Every XDR item occupies a multiple of 4 bytes. If `n` bytes of data are written, `r = (4 - (n % 4)) % 4` zero bytes of padding follow it. This means a 3-byte string will have 1 padding byte appended; a 4-byte string needs none.

```
+--------+--------+---+--------+--------+---+--------+
| byte 0 | byte 1 |...| byte n |   0    |...|   0    |
+--------+--------+---+--------+--------+---+--------+
|<------ n bytes ------>|<----- r bytes (padding) --->|
```

### Integers (§4.1, §4.2, §4.5)

- Signed and unsigned 32-bit integers occupy exactly 4 bytes, MSB first.
- 64-bit integers (`i64`, `u64`) occupy 8 bytes, MSB first.
- Narrower Rust types (`i8`, `i16`, `u8`, `u16`) are promoted to 32 bits on the wire.

### Strings and opaque data (§4.10, §4.11)

Variable-length strings and byte blobs share the same layout: a 4-byte unsigned length `n`, followed by `n` bytes of data, followed by 0–3 bytes of zero-padding.

```
+-----+-----+-----+-----+------+------+---+------+-----+---+-----+
|         length n        |byte 0|byte 1|...|byte n-1| 0 |...| 0 |
+-----+-----+-----+-----+------+------+---+------+-----+---+-----+
|<-------  4 bytes  ----->|<---- n bytes ---->|<--- r bytes ---->|
```

### Discriminated unions / enums (§4.15)

Rust `enum` variants are encoded as a 4-byte discriminant (the serde variant index) followed by the encoded arm contents (nothing for unit variants, the inner value for newtype variants, fields consecutively for struct/tuple variants).

```
+---+---+---+---+---+---+---+---+
|   discriminant  |  implied arm |
+---+---+---+---+---+---+---+---+
```

### Optional-data (§4.19)

`Option<T>` is a discriminated union with two arms: `FALSE (0)` for `None` and `TRUE (1)` followed by the encoded `T` for `Some(T)`.

---

## NFS example

Here is a realistic NFS3 file-attribute structure (`FATTR3`) using the crate:

```rust
use serde::{Deserialize, Serialize};
use xdr_serde::{from_bytes, to_bytes};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum NfsFileType { Reg, Dir, Blk, Chr, Lnk, Sock, Fifo }

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Fattr3 {
    ftype:      NfsFileType,
    mode:       u32,
    nlink:      u32,
    uid:        u32,
    gid:        u32,
    size:       u64,
    used:       u64,
    rdev_major: u32,
    rdev_minor: u32,
    fsid:       u64,
    fileid:     u64,
    atime_sec:  u32,
    atime_nsec: u32,
    mtime_sec:  u32,
    mtime_nsec: u32,
    ctime_sec:  u32,
    ctime_nsec: u32,
}

let attr = Fattr3 {
    ftype: NfsFileType::Reg,
    mode: 0o644, nlink: 1, uid: 1000, gid: 1000,
    size: 12345, used: 16384,
    rdev_major: 0, rdev_minor: 0,
    fsid: 0xABCD_EF01_2345_6789,
    fileid: 1,
    atime_sec: 1700000000, atime_nsec: 0,
    mtime_sec: 1700000001, mtime_nsec: 500_000_000,
    ctime_sec: 1700000001, ctime_nsec: 500_000_000,
};

let bytes = to_bytes(&attr).unwrap();
assert_eq!(bytes.len() % 4, 0); // always 4-byte aligned

let decoded: Fattr3 = from_bytes(&bytes).unwrap();
assert_eq!(attr, decoded);
```
