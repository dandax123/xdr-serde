# xdr-serde

A pure-Rust implementation of [XDR (eXternal Data Representation, RFC 4506)](https://datatracker.ietf.org/doc/html/rfc4506) serialization and deserialization, built on the [`serde`](https://serde.rs) framework.

XDR is the binary wire encoding used by ONC RPC protocols, most notably NFS. All values are big-endian (network byte order), and every encoded item is padded to a multiple of 4 bytes.

---

## Features

- Full coverage of all RFC 4506 data types
- Idiomatic `serde` integration — use `#[derive(Serialize, Deserialize)]` on your types
- `#[serde(with = "xdr_serde::fixed_opaque")]` for RFC 4506 §4.9 fixed-length opaque fields (`[u8; N]`)
- Zero-copy deserialization for strings and byte slices via lifetime-bounded `&'de [u8]`
- `to_writer` / `from_reader` for streaming I/O directly to/from sockets, files, or any `Write`/`Read` implementor
- `from_bytes_partial` for framing use cases — returns remaining unconsumed bytes
- No unsafe code
- No dependencies beyond `serde` itself

---

## Installation

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

let fh = FileHandle { inode: 0x0102030405060708, generation: 42, flags: 0 };

let bytes = to_bytes(&fh).unwrap();
assert_eq!(bytes.len(), 16); // 8 (u64) + 4 (u32) + 4 (u32)

let decoded: FileHandle = from_bytes(&bytes).unwrap();
assert_eq!(fh, decoded);
```

---

## API

### In-memory serialization

```rust
pub fn to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>>
```

Serializes any `serde`-compatible value into a freshly allocated `Vec<u8>` of XDR bytes.

### In-memory deserialization

```rust
pub fn from_bytes<T: DeserializeOwned>(input: &[u8]) -> Result<T>
```

Deserializes a value from a complete XDR byte slice.

```rust
pub fn from_bytes_partial<'de, T: Deserialize<'de>>(input: &'de [u8]) -> Result<(T, &'de [u8])>
```

Deserializes one value and returns the remaining unconsumed bytes alongside it. Useful when peeling values off a framed packet buffer one at a time.

```rust
let mut buf = to_bytes(&1u32).unwrap();
buf.extend(to_bytes(&2u32).unwrap());
buf.extend([0xFF, 0xFF]); // trailing bytes

let (first, rest) = from_bytes_partial::<u32>(&buf).unwrap(); // 1
let (second, tail) = from_bytes_partial::<u32>(rest).unwrap(); // 2
assert_eq!(tail, [0xFF, 0xFF]);
```

### Streaming serialization

```rust
pub fn to_writer<W: Write, T: Serialize>(writer: W, value: &T) -> Result<()>
```

Serializes directly into any `std::io::Write` implementor. No intermediate buffer is allocated. Use this when writing to a `TcpStream`, `File`, `BufWriter`, etc.

```rust
use std::io::BufWriter;
use std::net::TcpStream;

let stream = TcpStream::connect("127.0.0.1:2049")?;
to_writer(BufWriter::new(stream), &my_rpc_call)?;
```

`to_writer` and `to_bytes` are guaranteed to produce identical byte sequences for all inputs.

### Streaming deserialization

```rust
pub fn from_reader<R: Read, T: DeserializeOwned>(reader: R) -> Result<T>
```

Deserializes a value from any `std::io::Read` implementor. Only the bytes needed to decode `T` are consumed — `reader` is not read to exhaustion. For sockets or files, wrap with `BufReader` for better performance.

```rust
use std::io::BufReader;
use std::net::TcpListener;

let listener = TcpListener::bind("0.0.0.0:2049")?;
let (stream, _) = listener.accept()?;
let call: MyRpcCall = from_reader(BufReader::new(stream))?;
```

---

## Fixed-length opaque data (`[u8; N]`)

### The problem: §4.12 vs §4.9

XDR has two distinct encodings for byte arrays, and the right choice depends on what the array _means_:

**§4.12 — Fixed-length array** is the general case for arrays of typed elements. Each element is encoded individually, and for `u8` elements that means each byte is widened to a 4-byte XDR unsigned int. A `[u8; 12]` field encoded this way occupies **48 bytes**.

**§4.9 — Fixed-length opaque** is for raw, uninterpreted byte buffers of a statically known size. The `N` bytes are written directly to the wire, followed by 0–3 zero-padding bytes to reach a 4-byte boundary — no length prefix, no per-byte widening. A `[u8; 12]` field encoded this way occupies **12 bytes**.

Because serde models `[u8; N]` as a tuple, the XDR serializer will use §4.12 by default. For fields that are protocol-defined fixed-size blobs (cryptographic verifiers, file handles, state tokens, checksums), this produces wire output that is 4× too large and incompatible with any other XDR implementation.

### The fix: `#[serde(with = "xdr_serde::fixed_opaque")]`

Annotate any `[u8; N]` field to opt into §4.9 encoding:

```rust
use serde::{Deserialize, Serialize};
use xdr_serde::{from_bytes, to_bytes};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct StateId {
    pub sequence_id: u32,
    #[serde(with = "xdr_serde::fixed_opaque")]
    pub other: [u8; 12],
}

let id = StateId { sequence_id: 7, other: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] };

let bytes = to_bytes(&id).unwrap();
// sequence_id:  4 bytes (u32)
// other:       12 bytes (raw, §4.9 opaque — no length prefix, no padding since 12%4==0)
// Total:       16 bytes
assert_eq!(bytes.len(), 16);
assert_eq!(&bytes[..4],  [0, 0, 0, 7]);
assert_eq!(&bytes[4..], [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

let decoded: StateId = from_bytes(&bytes).unwrap();
assert_eq!(id, decoded);
```

### Size comparison

| Encoding                                     | RFC section              | Wire size for `[u8; 12]`        |
| -------------------------------------------- | ------------------------ | ------------------------------- |
| Default (`[u8; 12]` as tuple)                | §4.12 fixed-length array | **48 bytes** (12 × 4-byte uint) |
| `#[serde(with = "xdr_serde::fixed_opaque")]` | §4.9 fixed-length opaque | **12 bytes** (raw + 0 padding)  |

### Padding for non-multiples of 4

When `N` is not a multiple of 4, the serializer appends zero bytes so the total is 4-byte aligned, matching the XDR §3 block-size requirement:

```rust
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Packet {
    #[serde(with = "xdr_serde::fixed_opaque")]
    tag: [u8; 5],  // 5 bytes data + 3 zero bytes padding = 8 bytes on wire
    value: u32,
}
```

```text
 §4.9 fixed-length opaque [u8; 5]        u32
+----+----+----+----+----+----+----+----+----+----+----+----+
| b0 | b1 | b2 | b3 | b4 |  0 |  0 |  0 |       value     |
+----+----+----+----+----+----+----+----+----+----+----+----+
 <------- 5 data --------> <3 pad>
```

`fixed_opaque` handles all sizes from `[u8; 0]` through `[u8; N]` and works correctly via both `from_bytes` and `from_reader`.

---

## NFS example: NFSv4 stateid

The NFSv4 `stateid4` type (RFC 7530 §16.2.3) is a real-world example that requires `fixed_opaque`:

```rust
use serde::{Deserialize, Serialize};
use xdr_serde::{from_reader, to_writer};

/// NFSv4 stateid — seqid (4 bytes) + other (12 raw bytes) = 16 bytes total
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct StateId4 {
    pub seqid: u32,
    #[serde(with = "xdr_serde::fixed_opaque")]
    pub other: [u8; 12],
}

let state = StateId4 { seqid: 1, other: [0xAA; 12] };

// Write directly to a socket/file
let mut buf = Vec::new();
to_writer(&mut buf, &state).unwrap();
assert_eq!(buf.len(), 16); // exactly 16 bytes per the NFSv4 spec

// Read back from any reader
let decoded: StateId4 = from_reader(std::io::Cursor::new(&buf)).unwrap();
assert_eq!(state, decoded);
```

---

## XDR type mapping

| XDR type (RFC 4506)   | Rust / serde type             | Wire size                                                      |
| --------------------- | ----------------------------- | -------------------------------------------------------------- |
| `int`                 | `i8`, `i16`, `i32`            | 4 bytes (sign-extended)                                        |
| `unsigned int`        | `u8`, `u16`, `u32`            | 4 bytes (zero-extended)                                        |
| `hyper`               | `i64`                         | 8 bytes                                                        |
| `unsigned hyper`      | `u64`                         | 8 bytes                                                        |
| `float`               | `f32`                         | 4 bytes (IEEE 754)                                             |
| `double`              | `f64`                         | 8 bytes (IEEE 754)                                             |
| `bool`                | `bool`                        | 4 bytes (0 = false, 1 = true)                                  |
| `string`              | `String`, `&str`              | 4-byte length + data + 0–3 padding                             |
| opaque variable       | `Vec<u8>`, `&[u8]`            | 4-byte length + data + 0–3 padding                             |
| opaque fixed (§4.9)   | `[u8; N]` with `fixed_opaque` | N bytes + 0–3 padding (no length prefix, no per-byte widening) |
| optional-data         | `Option<T>`                   | 4-byte bool discriminant + encoded `T`                         |
| void                  | `()`, unit struct             | 0 bytes                                                        |
| unit enum variant     | unit enum variant             | 4-byte unsigned discriminant                                   |
| discriminated union   | enum with data                | 4-byte discriminant + encoded arm                              |
| structure             | struct                        | fields encoded consecutively, no length prefix                 |
| fixed-length array    | tuple, tuple struct           | elements consecutively, no length prefix                       |
| variable-length array | `Vec<T>`, seq                 | 4-byte count + elements                                        |
| map                   | `HashMap`, `BTreeMap`         | 4-byte pair count + key/value pairs                            |

---

## Error handling

All errors are variants of `xdr_serde::Error`:

| Variant                       | When it occurs                                                               |
| ----------------------------- | ---------------------------------------------------------------------------- |
| `UnexpectedEof`               | Input buffer or reader ended before the value was fully decoded              |
| `LengthRequired`              | Serializing a sequence with no known length (`serialize_seq(None)`)          |
| `InvalidString`               | String bytes were not valid UTF-8                                            |
| `InvalidBool(u32)`            | Boolean discriminant was neither `0` nor `1`                                 |
| `InvalidOption(u32)`          | Optional-data discriminant was neither `0` nor `1`                           |
| `InvalidDiscriminant(i32)`    | Enum discriminant did not match any known variant                            |
| `LengthOverflow { max, got }` | Encoded length exceeded the declared maximum                                 |
| `InvalidPadding`              | Padding bytes were non-zero                                                  |
| `Unsupported(&str)`           | The serde data model type has no XDR representation (e.g. `deserialize_any`) |
| `Io(String)`                  | An I/O error occurred during `to_writer` or `from_reader`                    |
| `Message(String)`             | A custom error propagated from a `serde` `Visitor`                           |

---

## Limitations

**XDR is not self-describing.** Unlike JSON or MessagePack, XDR has no type tags in the wire format — the receiver must know the schema ahead of time. As a result, `deserialize_any` and `deserialize_ignored_any` are not supported; you must always deserialize into a concrete Rust type.

**Sequence lengths must be known at serialization time.** When serializing a `Vec` or other sequence, serde calls `serialize_seq(len)`. If the length is `None` (e.g. from a lazy iterator), the serializer returns `Error::LengthRequired`. Always collect into a `Vec` first if needed.

**`fixed_opaque` is only for `[u8; N]` (§4.9 opaque data).** The `XdrFixedOpaque` trait is sealed and only `[u8; N]` implements it. It is specifically for fields that are raw byte blobs in the protocol — verifiers, file handles, tokens, and so on. For fixed-length arrays of typed elements (§4.12), use the normal serde field encoding, which will encode each element individually.

**Quadruple-precision floats are not directly supported.** RFC 4506 §4.8 defines a 128-bit float, but Rust has no native `f128` type. Quadruples can be handled as a `#[serde(with = "xdr_serde::fixed_opaque")] [u8; 16]` field if required.

---

## RFC 4506 compliance

| Section | XDR type                                                                |
| ------- | ----------------------------------------------------------------------- |
| §4.1    | Integer (signed 32-bit)                                                 |
| §4.2    | Unsigned Integer                                                        |
| §4.3    | Enumeration                                                             |
| §4.4    | Boolean                                                                 |
| §4.5    | Hyper Integer and Unsigned Hyper Integer (64-bit)                       |
| §4.6    | Floating-Point (IEEE 754 single)                                        |
| §4.7    | Double-Precision Floating-Point (IEEE 754 double)                       |
| §4.9    | Fixed-Length Opaque Data (`#[serde(with = "xdr_serde::fixed_opaque")]`) |
| §4.10   | Variable-Length Opaque Data                                             |
| §4.11   | String                                                                  |
| §4.12   | Fixed-Length Array (via tuples/tuple structs)                           |
| §4.13   | Variable-Length Array                                                   |
| §4.14   | Structure                                                               |
| §4.15   | Discriminated Union                                                     |
| §4.16   | Void                                                                    |

---

## License

Licensed under [MIT License](LICENSE).
