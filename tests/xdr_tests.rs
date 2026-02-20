use serde::{Deserialize, Serialize};
use xdr_serde::{from_bytes, to_bytes};

// ── Primitive round-trip tests ─────────────────────────────────────────────

#[test]
fn test_bool_true() {
    let bytes = to_bytes(&true).unwrap();
    assert_eq!(bytes, [0, 0, 0, 1]);
    let decoded: bool = from_bytes(&bytes).unwrap();
    assert!(decoded);
}

#[test]
fn test_bool_false() {
    let bytes = to_bytes(&false).unwrap();
    assert_eq!(bytes, [0, 0, 0, 0]);
    let decoded: bool = from_bytes(&bytes).unwrap();
    assert!(!decoded);
}

#[test]
fn test_i32_positive() {
    let v: i32 = 1234567;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes.len(), 4);
    let decoded: i32 = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_i32_negative() {
    let v: i32 = -42;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes.len(), 4);
    let decoded: i32 = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_i32_min_max() {
    for v in [i32::MIN, -1, 0, 1, i32::MAX] {
        let bytes = to_bytes(&v).unwrap();
        let decoded: i32 = from_bytes(&bytes).unwrap();
        assert_eq!(v, decoded, "failed for {}", v);
    }
}

#[test]
fn test_u32() {
    let v: u32 = 0xDEADBEEF;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0xDE, 0xAD, 0xBE, 0xEF]);
    let decoded: u32 = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_i64_hyper() {
    let v: i64 = -9_000_000_000;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes.len(), 8);
    let decoded: i64 = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_u64_unsigned_hyper() {
    let v: u64 = 0x0102030405060708;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [1, 2, 3, 4, 5, 6, 7, 8]);
    let decoded: u64 = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_f32() {
    let v: f32 = std::f32::consts::PI;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes.len(), 4);
    let decoded: f32 = from_bytes(&bytes).unwrap();
    assert_eq!(v.to_bits(), decoded.to_bits());
}

#[test]
fn test_f64() {
    let v: f64 = std::f64::consts::E;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes.len(), 8);
    let decoded: f64 = from_bytes(&bytes).unwrap();
    assert_eq!(v.to_bits(), decoded.to_bits());
}

#[test]
fn test_f32_special_values() {
    for v in [
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        0.0_f32,
        -0.0_f32,
    ] {
        let bytes = to_bytes(&v).unwrap();
        let decoded: f32 = from_bytes(&bytes).unwrap();
        assert_eq!(v.to_bits(), decoded.to_bits());
    }
}

// ── String tests ───────────────────────────────────────────────────────────

#[test]
fn test_string_no_padding() {
    // "ABCD" = 4 bytes, no padding needed
    let v = "ABCD".to_string();
    let bytes = to_bytes(&v).unwrap();
    // 4 (length) + 4 (data) = 8
    assert_eq!(bytes.len(), 8);
    assert_eq!(&bytes[..4], [0, 0, 0, 4]); // length
    assert_eq!(&bytes[4..], b"ABCD");
    let decoded: String = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_string_with_padding() {
    // "Hi" = 2 bytes → needs 2 bytes of padding
    let v = "Hi".to_string();
    let bytes = to_bytes(&v).unwrap();
    // 4 (length) + 2 (data) + 2 (padding) = 8
    assert_eq!(bytes.len(), 8);
    assert_eq!(&bytes[..4], [0, 0, 0, 2]);
    assert_eq!(&bytes[4..6], b"Hi");
    assert_eq!(&bytes[6..8], [0, 0]);
    let decoded: String = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_empty_string() {
    let v = "".to_string();
    let bytes = to_bytes(&v).unwrap();
    // 4 (length = 0) + 0 (data) = 4
    assert_eq!(bytes, [0, 0, 0, 0]);
    let decoded: String = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_string_length_1() {
    let v = "X".to_string();
    let bytes = to_bytes(&v).unwrap();
    // 4 (length) + 1 + 3 padding = 8
    assert_eq!(bytes.len(), 8);
    let decoded: String = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_string_length_3() {
    let v = "foo".to_string();
    let bytes = to_bytes(&v).unwrap();
    // 4 (length) + 3 + 1 padding = 8
    assert_eq!(bytes.len(), 8);
    let decoded: String = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ── Byte slice / opaque tests ──────────────────────────────────────────────

#[test]
fn test_bytes_variable_opaque() {
    let v: Vec<u8> = vec![0xAA, 0xBB, 0xCC];
    let bytes = to_bytes(&serde_bytes::ByteBuf::from(v.clone())).unwrap();
    // 4 (len=3) + 3 + 1 padding = 8
    assert_eq!(bytes.len(), 8);
    let decoded: serde_bytes::ByteBuf = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded.into_vec());
}

// ── Option tests ───────────────────────────────────────────────────────────

#[test]
fn test_option_none() {
    let v: Option<u32> = None;
    let bytes = to_bytes(&v).unwrap();
    // 4 bytes: discriminant = 0
    assert_eq!(bytes, [0, 0, 0, 0]);
    let decoded: Option<u32> = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_option_some() {
    let v: Option<u32> = Some(42);
    let bytes = to_bytes(&v).unwrap();
    // 4 (discriminant=1) + 4 (value=42) = 8
    assert_eq!(bytes.len(), 8);
    assert_eq!(&bytes[..4], [0, 0, 0, 1]);
    assert_eq!(&bytes[4..], [0, 0, 0, 42]);
    let decoded: Option<u32> = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ── Struct tests ───────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FileHandle {
    inode: u64,
    generation: u32,
    flags: u32,
}

#[test]
fn test_struct_file_handle() {
    let fh = FileHandle {
        inode: 0x0102030405060708,
        generation: 42,
        flags: 0xFFFF,
    };
    let bytes = to_bytes(&fh).unwrap();
    // 8 + 4 + 4 = 16 bytes total
    assert_eq!(bytes.len(), 16);
    assert_eq!(&bytes[..8], [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(&bytes[8..12], [0, 0, 0, 42]);
    assert_eq!(&bytes[12..], [0, 0, 0xFF, 0xFF]);
    let decoded: FileHandle = from_bytes(&bytes).unwrap();
    assert_eq!(fh, decoded);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct NfsAttr {
    file_type: u32,
    mode: u32,
    nlink: u32,
    uid: u32,
    gid: u32,
    size: u64,
    name: String,
    active: bool,
}

#[test]
fn test_complex_struct() {
    let attr = NfsAttr {
        file_type: 1,
        mode: 0o755,
        nlink: 2,
        uid: 1000,
        gid: 1000,
        size: 4096,
        name: "file.txt".to_string(),
        active: true,
    };
    let bytes = to_bytes(&attr).unwrap();
    let decoded: NfsAttr = from_bytes(&bytes).unwrap();
    assert_eq!(attr, decoded);
}

// ── Enum tests ─────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Color {
    Red,   // index 0
    Green, // index 1
    Blue,  // index 2
}

#[test]
fn test_unit_enum() {
    for (color, expected_idx) in [(Color::Red, 0u32), (Color::Green, 1), (Color::Blue, 2)] {
        let bytes = to_bytes(&color).unwrap();
        assert_eq!(bytes.len(), 4);
        let idx = u32::from_be_bytes(bytes[..4].try_into().unwrap());
        assert_eq!(idx, expected_idx);
        let decoded: Color = from_bytes(&bytes).unwrap();
        assert_eq!(color, decoded);
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum FileType {
    Regular,                             // 0
    Directory,                           // 1
    Symlink(String),                     // 2 + target
    BlockDev { major: u32, minor: u32 }, // 3 + two u32s
}

#[test]
fn test_newtype_enum_variant() {
    let v = FileType::Symlink("/etc/hosts".to_string());
    let bytes = to_bytes(&v).unwrap();
    // 4 (discriminant=2) + 4 (len=10) + 10 + 2 padding = 20
    assert_eq!(bytes.len(), 20);
    assert_eq!(&bytes[..4], [0, 0, 0, 2]);
    let decoded: FileType = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_struct_enum_variant() {
    let v = FileType::BlockDev { major: 8, minor: 1 };
    let bytes = to_bytes(&v).unwrap();
    // 4 (discriminant=3) + 4 + 4 = 12
    assert_eq!(bytes.len(), 12);
    assert_eq!(&bytes[..4], [0, 0, 0, 3]);
    assert_eq!(&bytes[4..8], [0, 0, 0, 8]);
    assert_eq!(&bytes[8..12], [0, 0, 0, 1]);
    let decoded: FileType = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_unit_enum_variants() {
    let v = FileType::Regular;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0, 0, 0, 0]);
    let decoded: FileType = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);

    let v = FileType::Directory;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0, 0, 0, 1]);
    let decoded: FileType = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ── Sequence tests ─────────────────────────────────────────────────────────

#[test]
fn test_vec_u32() {
    let v: Vec<u32> = vec![1, 2, 3, 4, 5];
    let bytes = to_bytes(&v).unwrap();
    // 4 (count=5) + 5*4 = 24
    assert_eq!(bytes.len(), 24);
    assert_eq!(&bytes[..4], [0, 0, 0, 5]); // count prefix
    let decoded: Vec<u32> = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_empty_vec() {
    let v: Vec<u32> = vec![];
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0, 0, 0, 0]);
    let decoded: Vec<u32> = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_vec_of_strings() {
    let v: Vec<String> = vec!["hello".to_string(), "world".to_string()];
    let bytes = to_bytes(&v).unwrap();
    let decoded: Vec<String> = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ── Tuple tests ────────────────────────────────────────────────────────────

#[test]
fn test_tuple_no_count_prefix() {
    // Tuples are fixed-length: no count prefix in XDR
    let v: (u32, u32, u32) = (1, 2, 3);
    let bytes = to_bytes(&v).unwrap();
    // 3 * 4 = 12 (no length prefix)
    assert_eq!(bytes.len(), 12);
    assert_eq!(&bytes[..4], [0, 0, 0, 1]);
    assert_eq!(&bytes[4..8], [0, 0, 0, 2]);
    assert_eq!(&bytes[8..12], [0, 0, 0, 3]);
    let decoded: (u32, u32, u32) = from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ── Void / unit tests ──────────────────────────────────────────────────────

#[test]
fn test_unit_void() {
    let bytes = to_bytes(&()).unwrap();
    assert_eq!(bytes.len(), 0); // XDR void = 0 bytes
    let decoded: () = from_bytes(&bytes).unwrap();
    assert_eq!((), decoded);
}

// ── Byte-level encoding verification ──────────────────────────────────────

#[test]
fn test_xdr_int_big_endian() {
    // RFC 4506 §4.1: MSB is byte 0
    let v: i32 = 1; // 0x00000001
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0x00, 0x00, 0x00, 0x01]);
}

#[test]
fn test_xdr_unsigned_int_big_endian() {
    let v: u32 = 0x12345678;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0x12, 0x34, 0x56, 0x78]);
}

#[test]
fn test_xdr_hyper_big_endian() {
    let v: i64 = 1; // 0x0000000000000001
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0, 0, 0, 0, 0, 0, 0, 1]);
}

#[test]
fn test_string_padding_correctness() {
    // XDR §4.11: n bytes + r zero bytes where (n + r) mod 4 == 0
    for (s, expected_total) in [
        ("", 4usize),  // 4 len + 0 bytes + 0 pad
        ("A", 8),      // 4 len + 1 byte + 3 pad
        ("AB", 8),     // 4 len + 2 bytes + 2 pad
        ("ABC", 8),    // 4 len + 3 bytes + 1 pad
        ("ABCD", 8),   // 4 len + 4 bytes + 0 pad
        ("ABCDE", 12), // 4 len + 5 bytes + 3 pad
    ] {
        let bytes = to_bytes(&s.to_string()).unwrap();
        assert_eq!(
            bytes.len(),
            expected_total,
            "string {:?} expected {} bytes, got {}",
            s,
            expected_total,
            bytes.len()
        );
        // All bytes after the data should be zero-padded
        let data_len = s.len();
        let pad_start = 4 + data_len;
        for &b in &bytes[pad_start..] {
            assert_eq!(b, 0, "padding byte should be zero for {:?}", s);
        }
    }
}

// ── Nested structures ──────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct LookupResult {
    found: bool,
    handle: Option<FileHandle>,
    name: String,
}

#[test]
fn test_nested_struct_with_option() {
    let r = LookupResult {
        found: true,
        handle: Some(FileHandle {
            inode: 99,
            generation: 1,
            flags: 0,
        }),
        name: "test".to_string(),
    };
    let bytes = to_bytes(&r).unwrap();
    let decoded: LookupResult = from_bytes(&bytes).unwrap();
    assert_eq!(r, decoded);
}

#[test]
fn test_nested_struct_option_none() {
    let r = LookupResult {
        found: false,
        handle: None,
        name: "missing".to_string(),
    };
    let bytes = to_bytes(&r).unwrap();
    let decoded: LookupResult = from_bytes(&bytes).unwrap();
    assert_eq!(r, decoded);
}

// ── Error cases ────────────────────────────────────────────────────────────

#[test]
fn test_unexpected_eof() {
    let result: xdr_serde::Result<u32> = from_bytes(&[0, 0, 0]); // only 3 bytes
    assert!(result.is_err());
    matches!(result.unwrap_err(), xdr_serde::Error::UnexpectedEof);
}

#[test]
fn test_invalid_bool() {
    let invalid = [0, 0, 0, 2]; // bool must be 0 or 1
    let result: xdr_serde::Result<bool> = from_bytes(&invalid);
    assert!(result.is_err());
}

#[test]
fn test_partial_deserialization() {
    // Two u32 values encoded back to back
    let mut bytes = to_bytes(&42u32).unwrap();
    bytes.extend(to_bytes(&99u32).unwrap());
    bytes.extend([0xFF, 0xFF]); // trailing bytes

    let (first, rest) = xdr_serde::from_bytes_partial::<u32>(&bytes).unwrap();
    assert_eq!(first, 42);
    let (second, remaining) = xdr_serde::from_bytes_partial::<u32>(rest).unwrap();
    assert_eq!(second, 99);
    assert_eq!(remaining, [0xFF, 0xFF]);
}

// ── NFS-style realistic test ───────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum NfsFileType {
    Reg = 0,
    Dir = 1,
    Blk = 2,
    Chr = 3,
    Lnk = 4,
    Sock = 5,
    Fifo = 6,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Fattr3 {
    ftype: NfsFileType,
    mode: u32,
    nlink: u32,
    uid: u32,
    gid: u32,
    size: u64,
    used: u64,
    rdev_major: u32,
    rdev_minor: u32,
    fsid: u64,
    fileid: u64,
    atime_sec: u32,
    atime_nsec: u32,
    mtime_sec: u32,
    mtime_nsec: u32,
    ctime_sec: u32,
    ctime_nsec: u32,
}

#[test]
fn test_nfs_fattr3_roundtrip() {
    let attr = Fattr3 {
        ftype: NfsFileType::Reg,
        mode: 0o644,
        nlink: 1,
        uid: 1000,
        gid: 1000,
        size: 12345,
        used: 16384,
        rdev_major: 0,
        rdev_minor: 0,
        fsid: 0xABCD_EF01_2345_6789,
        fileid: 0x0000_0000_0000_0001,
        atime_sec: 1700000000,
        atime_nsec: 0,
        mtime_sec: 1700000001,
        mtime_nsec: 500000000,
        ctime_sec: 1700000001,
        ctime_nsec: 500000000,
    };
    let bytes = to_bytes(&attr).unwrap();
    // All XDR fields are 4-byte aligned
    assert_eq!(bytes.len() % 4, 0);
    let decoded: Fattr3 = from_bytes(&bytes).unwrap();
    assert_eq!(attr, decoded);
}
