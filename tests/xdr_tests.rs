use serde::{Deserialize, Serialize};
use xdr_serde::{from_bytes, from_bytes_partial, from_reader, to_bytes, to_writer};


#[test]
fn test_bool_true() {
    let bytes = to_bytes(&true).unwrap();
    assert_eq!(bytes, [0, 0, 0, 1]);
    assert!(from_bytes::<bool>(&bytes).unwrap());
}

#[test]
fn test_bool_false() {
    let bytes = to_bytes(&false).unwrap();
    assert_eq!(bytes, [0, 0, 0, 0]);
    assert!(!from_bytes::<bool>(&bytes).unwrap());
}

#[test]
fn test_i32_min_max() {
    for v in [i32::MIN, -1, 0, 1, i32::MAX] {
        assert_eq!(v, from_bytes::<i32>(&to_bytes(&v).unwrap()).unwrap());
    }
}

#[test]
fn test_u32_big_endian() {
    let bytes = to_bytes(&0xDEADBEEFu32).unwrap();
    assert_eq!(bytes, [0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn test_i64_hyper() {
    let v: i64 = -9_000_000_000;
    assert_eq!(v, from_bytes::<i64>(&to_bytes(&v).unwrap()).unwrap());
}

#[test]
fn test_u64_unsigned_hyper() {
    let bytes = to_bytes(&0x0102030405060708u64).unwrap();
    assert_eq!(bytes, [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_f32_roundtrip() {
    for v in [std::f32::consts::PI, f32::INFINITY, f32::NAN, 0.0_f32, -0.0_f32] {
        let decoded: f32 = from_bytes(&to_bytes(&v).unwrap()).unwrap();
        assert_eq!(v.to_bits(), decoded.to_bits());
    }
}

#[test]
fn test_f64_roundtrip() {
    let v = std::f64::consts::E;
    let decoded: f64 = from_bytes(&to_bytes(&v).unwrap()).unwrap();
    assert_eq!(v.to_bits(), decoded.to_bits());
}

#[test]
fn test_string_padding() {
    for (s, total) in [("", 4usize), ("A", 8), ("AB", 8), ("ABC", 8), ("ABCD", 8), ("ABCDE", 12)] {
        let bytes = to_bytes(&s.to_string()).unwrap();
        assert_eq!(bytes.len(), total, "string {:?}", s);
        let pad_start = 4 + s.len();
        for &b in &bytes[pad_start..] {
            assert_eq!(b, 0, "non-zero pad for {:?}", s);
        }
        assert_eq!(s.to_string(), from_bytes::<String>(&bytes).unwrap());
    }
}

#[test]
fn test_option_none_some() {
    assert_eq!(to_bytes(&Option::<u32>::None).unwrap(), [0,0,0,0]);
    let bytes = to_bytes(&Some(42u32)).unwrap();
    assert_eq!(bytes, [0,0,0,1, 0,0,0,42]);
    assert_eq!(Some(42u32), from_bytes::<Option<u32>>(&bytes).unwrap());
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FileHandle { inode: u64, generation: u32, flags: u32 }

#[test]
fn test_struct_file_handle() {
    let fh = FileHandle { inode: 0x0102030405060708, generation: 42, flags: 0xFFFF };
    let bytes = to_bytes(&fh).unwrap();
    assert_eq!(bytes.len(), 16);
    assert_eq!(fh, from_bytes(&bytes).unwrap());
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum FileType {
    Regular, Directory, Symlink(String), BlockDevice { major: u32, minor: u32 }
}

#[test]
fn test_unit_enum() {
    let bytes = to_bytes(&FileType::Regular).unwrap();
    assert_eq!(bytes, [0,0,0,0]);
    assert_eq!(FileType::Regular, from_bytes(&bytes).unwrap());
}

#[test]
fn test_newtype_enum_variant() {
    let v = FileType::Symlink("/etc/hosts".to_string());
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(&bytes[..4], [0,0,0,2]); // discriminant
    assert_eq!(v, from_bytes(&bytes).unwrap());
}

#[test]
fn test_struct_enum_variant() {
    let v = FileType::BlockDevice { major: 8, minor: 1 };
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0,0,0,3, 0,0,0,8, 0,0,0,1]);
    assert_eq!(v, from_bytes(&bytes).unwrap());
}

#[test]
fn test_vec_u32() {
    let v: Vec<u32> = vec![1, 2, 3, 4, 5];
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(&bytes[..4], [0,0,0,5]); // count prefix
    assert_eq!(bytes.len(), 24);
    assert_eq!(v, from_bytes::<Vec<u32>>(&bytes).unwrap());
}

#[test]
fn test_tuple_no_count_prefix() {
    let v: (u32, u32, u32) = (1, 2, 3);
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes.len(), 12); // no length prefix
    assert_eq!(bytes, [0,0,0,1, 0,0,0,2, 0,0,0,3]);
    assert_eq!(v, from_bytes(&bytes).unwrap());
}

#[test]
fn test_unit_void() {
    assert_eq!(to_bytes(&()).unwrap().len(), 0);
    from_bytes::<()>(&[]).unwrap();
}

#[test]
fn test_error_unexpected_eof() {
    let result = from_bytes::<u32>(&[0, 0, 0]); // 3 bytes instead of 4
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), xdr_serde::Error::UnexpectedEof));
}

#[test]
fn test_error_invalid_bool() {
    let result = from_bytes::<bool>(&[0, 0, 0, 2]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), xdr_serde::Error::InvalidBool(2)));
}

#[test]
fn test_partial_deserialization() {
    let mut buf = to_bytes(&42u32).unwrap();
    buf.extend(to_bytes(&99u32).unwrap());
    buf.extend([0xFF, 0xFF]);
    let (first, rest) = from_bytes_partial::<u32>(&buf).unwrap();
    assert_eq!(first, 42);
    let (second, remaining) = from_bytes_partial::<u32>(rest).unwrap();
    assert_eq!(second, 99);
    assert_eq!(remaining, [0xFF, 0xFF]);
}

// ══════════════════════════════════════════════════════════════════════════
// NEW: to_writer / from_reader tests
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_to_writer_vec() {
    let mut buf = Vec::new();
    to_writer(&mut buf, &42u32).unwrap();
    assert_eq!(buf, [0, 0, 0, 42]);
}

#[test]
fn test_to_writer_matches_to_bytes() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Msg { id: u32, name: String, value: i64 }

    let msg = Msg { id: 7, name: "hello".into(), value: -9999 };
    let bytes = to_bytes(&msg).unwrap();
    let mut written = Vec::new();
    to_writer(&mut written, &msg).unwrap();
    assert_eq!(bytes, written, "to_writer must produce identical output to to_bytes");
}

#[test]
fn test_to_writer_cursor() {
    let mut cursor = std::io::Cursor::new(Vec::new());
    to_writer(&mut cursor, &0xDEADBEEFu32).unwrap();
    assert_eq!(cursor.into_inner(), [0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn test_from_reader_basic() {
    let bytes = to_bytes(&1234u32).unwrap();
    let decoded: u32 = from_reader(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(decoded, 1234);
}

#[test]
fn test_from_reader_struct() {
    let fh = FileHandle { inode: 99, generation: 3, flags: 0 };
    let bytes = to_bytes(&fh).unwrap();
    let decoded: FileHandle = from_reader(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(fh, decoded);
}

#[test]
fn test_from_reader_string() {
    let s = "hello world".to_string();
    let bytes = to_bytes(&s).unwrap();
    let decoded: String = from_reader(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(s, decoded);
}

#[test]
fn test_from_reader_vec() {
    let v: Vec<u32> = vec![10, 20, 30, 40];
    let bytes = to_bytes(&v).unwrap();
    let decoded: Vec<u32> = from_reader(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_from_reader_option_some() {
    let v: Option<u32> = Some(42);
    let bytes = to_bytes(&v).unwrap();
    let decoded: Option<u32> = from_reader(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_from_reader_enum() {
    let v = FileType::BlockDevice { major: 8, minor: 1 };
    let bytes = to_bytes(&v).unwrap();
    let decoded: FileType = from_reader(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn test_reader_eof_error() {
    let bytes = [0u8, 0, 0]; // 3 bytes — too short for a u32
    let result = from_reader::<_, u32>(std::io::Cursor::new(bytes));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), xdr_serde::Error::UnexpectedEof));
}

#[test]
fn test_to_writer_from_reader_roundtrip() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Record { seq: u64, tag: u32, name: String, active: bool }

    let rec = Record { seq: 0xABCD_EF01_2345_6789, tag: 99, name: "NFS4".into(), active: true };
    let mut buf = Vec::new();
    to_writer(&mut buf, &rec).unwrap();
    let decoded: Record = from_reader(std::io::Cursor::new(buf)).unwrap();
    assert_eq!(rec, decoded);
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct StateId {
    pub sequence_id: u32,
    #[serde(with = "xdr_serde::fixed_opaque")]
    pub other: [u8; 12],
}

#[test]
fn test_fixed_opaque_no_length_prefix() {
    let id = StateId { sequence_id: 7, other: [1,2,3,4,5,6,7,8,9,10,11,12] };
    let bytes = to_bytes(&id).unwrap();
    // 4 (u32) + 12 (raw bytes, no length prefix, 12%4==0 so no padding) = 16
    assert_eq!(bytes.len(), 16);
    // Verify the 4-byte sequence_id comes first
    assert_eq!(&bytes[..4], [0, 0, 0, 7]);
    // Verify the raw bytes immediately follow — NO 4-byte length prefix
    assert_eq!(&bytes[4..], [1,2,3,4,5,6,7,8,9,10,11,12]);
}

#[test]
fn test_fixed_opaque_roundtrip_12() {
    let id = StateId { sequence_id: 42, other: [0xFF; 12] };
    let decoded: StateId = from_bytes(&to_bytes(&id).unwrap()).unwrap();
    assert_eq!(id, decoded);
}

#[test]
fn test_fixed_opaque_roundtrip_via_reader() {
    let id = StateId { sequence_id: 1, other: [0xAA, 0xBB, 0xCC, 0xDD, 0, 0, 0, 0, 0, 0, 0, 0] };
    let bytes = to_bytes(&id).unwrap();
    let decoded: StateId = from_reader(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(id, decoded);
}

/// Contrast: without fixed_opaque, [u8; 12] serializes as 12 XDR u8s = 48 bytes.
#[test]
fn test_fixed_opaque_vs_default_size() {
    #[derive(Serialize, Deserialize)]
    struct DefaultArray { arr: [u8; 12] }

    let default_bytes = to_bytes(&DefaultArray { arr: [0u8; 12] }).unwrap();
    // serde default: serialize_tuple(12) → 12 elements, each as u8 = 4 bytes → 48 bytes
    assert_eq!(default_bytes.len(), 48, "default [u8;12] should be 48 bytes");

    let fixed_bytes = to_bytes(&StateId { sequence_id: 0, other: [0u8; 12] }).unwrap();
    // fixed_opaque: 4 (u32) + 12 raw = 16 bytes
    assert_eq!(fixed_bytes.len(), 16);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Nfs4FileId {
    #[serde(with = "xdr_serde::fixed_opaque")]
    pub verifier: [u8; 8],  // 8 bytes, no padding needed
    pub generation: u32,
    #[serde(with = "xdr_serde::fixed_opaque")]
    pub handle: [u8; 5],    // 5 bytes → 3 padding bytes
    pub flags: u32,
}

#[test]
fn test_fixed_opaque_with_padding() {
    // verifier: 8 bytes (8%4==0, no padding)
    // generation:      4 bytes (u32)
    // handle:   5 bytes + 3 padding = 8 bytes
    // flags:    4 bytes
    // Total: 8 + 4 + 8 + 4 = 24 bytes
    let fid = Nfs4FileId {
        verifier: [0xAA; 8],
        generation: 3,
        handle: [1, 2, 3, 4, 5],
        flags: 0,
    };
    let bytes = to_bytes(&fid).unwrap();
    assert_eq!(bytes.len(), 24);
    // verifier (8 raw)
    assert_eq!(&bytes[..8], [0xAA; 8]);
    // gen (4 bytes)
    assert_eq!(&bytes[8..12], [0, 0, 0, 3]);
    // handle (5 bytes + 3 pad)
    assert_eq!(&bytes[12..17], [1, 2, 3, 4, 5]);
    assert_eq!(&bytes[17..20], [0, 0, 0]); // padding
    // flags
    assert_eq!(&bytes[20..24], [0, 0, 0, 0]);

    let decoded: Nfs4FileId = from_bytes(&bytes).unwrap();
    assert_eq!(fid, decoded);
}

#[test]
fn test_fixed_opaque_zero_bytes() {
    // [u8; 0] is an edge case — zero bytes, zero padding
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Empty {
        x: u32,
        #[serde(with = "xdr_serde::fixed_opaque")]
        zero: [u8; 0],
    }
    let v = Empty { x: 1, zero: [] };
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes.len(), 4); // just the u32
    assert_eq!(v, from_bytes(&bytes).unwrap());
}

#[test]
fn test_fixed_opaque_4_bytes_no_padding() {
    // [u8; 4] — exactly one XDR block, no padding
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Cookie {
        #[serde(with = "xdr_serde::fixed_opaque")]
        val: [u8; 4],
    }
    let c = Cookie { val: [0xDE, 0xAD, 0xBE, 0xEF] };
    let bytes = to_bytes(&c).unwrap();
    assert_eq!(bytes, [0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(c, from_bytes(&bytes).unwrap());
}

#[test]
fn test_fixed_opaque_all_sizes_1_through_16() {
    // Verify that every array size from 1 to 16 encodes/decodes correctly
    // and the total wire size is always a multiple of 4.
    macro_rules! test_size {
        ($n:expr) => {{
            #[derive(Serialize, Deserialize, Debug, PartialEq)]
            struct W {
                #[serde(with = "xdr_serde::fixed_opaque")]
                data: [u8; $n],
            }
            let w = W { data: [0xABu8; $n] };
            let bytes = to_bytes(&w).unwrap();
            // Wire size must be multiple of 4
            assert_eq!(bytes.len() % 4, 0, "size {} not 4-byte aligned", $n);
            // Expected: ceil(n/4)*4 bytes
            let expected = ($n + 3) / 4 * 4;
            assert_eq!(bytes.len(), expected, "size {}", $n);
            // First n bytes are the data
            assert_eq!(&bytes[..$n], &[0xABu8; $n][..]);
            // Remaining bytes are zero-padding
            for &b in &bytes[$n..] {
                assert_eq!(b, 0, "non-zero pad for size {}", $n);
            }
            // Roundtrip
            assert_eq!(w, from_bytes::<W>(&bytes).unwrap(), "roundtrip size {}", $n);
        }};
    }
    test_size!(1);
    test_size!(2);
    test_size!(3);
    test_size!(4);
    test_size!(5);
    test_size!(6);
    test_size!(7);
    test_size!(8);
    test_size!(9);
    test_size!(10);
    test_size!(11);
    test_size!(12);
    test_size!(13);
    test_size!(14);
    test_size!(15);
    test_size!(16);
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum NfsFileType { Reg, Dir, Blk, Chr, Lnk, Sock, Fifo }

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Fattr3 {
    ftype: NfsFileType, mode: u32, nlink: u32, uid: u32, gid: u32,
    size: u64, used: u64, rdev_major: u32, rdev_minor: u32,
    fsid: u64, fileid: u64,
    atime_sec: u32, atime_nsec: u32, mtime_sec: u32, mtime_nsec: u32,
    ctime_sec: u32, ctime_nsec: u32,
}

#[test]
fn test_nfs_fattr3_roundtrip() {
    let attr = Fattr3 {
        ftype: NfsFileType::Reg, mode: 0o644, nlink: 1, uid: 1000, gid: 1000,
        size: 12345, used: 16384, rdev_major: 0, rdev_minor: 0,
        fsid: 0xABCD_EF01_2345_6789, fileid: 1,
        atime_sec: 1700000000, atime_nsec: 0, mtime_sec: 1700000001, mtime_nsec: 500000000,
        ctime_sec: 1700000001, ctime_nsec: 500000000,
    };
    let bytes = to_bytes(&attr).unwrap();
    assert_eq!(bytes.len() % 4, 0);
    assert_eq!(attr, from_bytes(&bytes).unwrap());
    // Also verify reader path
    assert_eq!(attr, from_reader(std::io::Cursor::new(&bytes[..])).unwrap());
}

/// NFS4 compound stateID — uses fixed_opaque for the 12-byte other field.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct NfsStateId {
    pub seqid: u32,
    #[serde(with = "xdr_serde::fixed_opaque")]
    pub other: [u8; 12],
}

#[test]
fn test_nfs_stateid_xdr_wire_layout() {
    let state = NfsStateId { seqid: 1, other: [0xAA; 12] };
    let bytes = to_bytes(&state).unwrap();
    // Per NFSv4 spec: seqid (4 bytes) + other (12 bytes) = 16 bytes total
    assert_eq!(bytes.len(), 16);
    assert_eq!(&bytes[..4],  [0, 0, 0, 1]);   // seqid
    assert_eq!(&bytes[4..16], [0xAA; 12]);     // other (raw, no length prefix)
    assert_eq!(state, from_bytes(&bytes).unwrap());
    assert_eq!(state, from_reader(std::io::Cursor::new(&bytes[..])).unwrap());
}