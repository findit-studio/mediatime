use super::*;

use ::buffa::{
  encoding::{WireType, encode_varint},
  types::{encode_int32, encode_int64},
};
use core::num::NonZeroI32;

const VARINT: u8 = WireType::Varint as u8;
const LEN: u8 = WireType::LengthDelimited as u8;

/// The reason an inverted range is refused with, as `DecodeError::Custom`
/// carries it.
const INVERTED_RANGE: &str = wire::ConversionError::InvertedRange.reason();

fn nz(n: i32) -> NonZeroI32 {
  NonZeroI32::new(n).unwrap()
}

// ---- Timebase ----

#[test]
fn timebase_default_instance_and_clear() {
  assert_eq!(
    *<Timebase as DefaultInstance>::default_instance(),
    Timebase::default()
  );
  let mut tb = Timebase::new(7, nz(9));
  Message::clear(&mut tb);
  assert_eq!(tb, Timebase::default());
}

#[test]
fn timebase_field1_wrong_wire_type_errors() {
  let mut buf: Vec<u8> = Vec::new();
  Tag::new(1, WireType::LengthDelimited).encode(&mut buf);
  encode_varint(0, &mut buf);
  let err = <Timebase as Message>::decode_from_slice(&buf).unwrap_err();
  assert!(
    matches!(err, DecodeError::WireTypeMismatch { field_number: 1, expected, actual }
        if expected == VARINT && actual == LEN),
    "got {err:?}"
  );
}

#[test]
fn timebase_field2_wrong_wire_type_errors() {
  let mut buf: Vec<u8> = Vec::new();
  Tag::new(1, WireType::Varint).encode(&mut buf);
  encode_int32(5, &mut buf);
  Tag::new(2, WireType::LengthDelimited).encode(&mut buf);
  encode_varint(0, &mut buf);
  let err = <Timebase as Message>::decode_from_slice(&buf).unwrap_err();
  assert!(
    matches!(err, DecodeError::WireTypeMismatch { field_number: 2, expected, actual }
        if expected == VARINT && actual == LEN),
    "got {err:?}"
  );
}

/// A timebase message with the given fields, each present.
fn timebase_fields(fields: &[(u32, i32)]) -> Vec<u8> {
  let mut buf: Vec<u8> = Vec::new();
  for &(field, value) in fields {
    Tag::new(field, WireType::Varint).encode(&mut buf);
    encode_int32(value, &mut buf);
  }
  buf
}

#[test]
fn an_absent_numerator_reads_as_zero() {
  // proto3 writes `0/3` as the denominator alone: the read starts from
  // protobuf's zero state, so the numerator it never saw is 0.
  let tb = <Timebase as Message>::decode_from_slice(&[0x10, 0x03]).unwrap();
  assert_eq!((tb.num(), tb.den().get()), (0, 3));
  let mut existing = Timebase::new(30_000, nz(1001));
  existing.merge_from_slice(&[0x10, 0x03]).unwrap();
  assert_eq!(
    (existing.num(), existing.den().get()),
    (0, 3),
    "replaced, not merged"
  );
}

#[test]
fn a_bad_timebase_is_refused_by_name_never_clamped() {
  // A peer writing `uint32` values above `i32::MAX` produces varints that
  // `decode_int32` truncates into the negative half; a zero or absent
  // denominator is no denominator. None of them is repaired.
  for (fields, reason) in [
    (&[(1, 7), (2, 0)][..], "timebase denominator is zero"),
    (&[(1, 7)][..], "timebase denominator is zero"),
    (&[(1, -7), (2, 9)][..], "timebase numerator is negative"),
    (&[(1, 7), (2, -9)][..], "timebase denominator is negative"),
  ] {
    let err = <Timebase as Message>::decode_from_slice(&timebase_fields(fields)).unwrap_err();
    assert!(
      matches!(err, DecodeError::Custom(r) if r == reason),
      "{fields:?}: {err:?}"
    );
  }
  // A refused read leaves the value it was merged into as it was.
  let mut existing = Timebase::new(1, nz(90_000));
  assert!(
    existing
      .merge_from_slice(&timebase_fields(&[(2, 0)]))
      .is_err()
  );
  assert_eq!(existing, Timebase::new(1, nz(90_000)));
}

#[test]
fn a_timestamp_or_range_without_a_timebase_is_refused() {
  for err in [
    <Timestamp as Message>::decode_from_slice(&[0x08, 0x05]).unwrap_err(),
    <TimeRange as Message>::decode_from_slice(&[0x10, 0x05]).unwrap_err(),
  ] {
    assert!(
      matches!(err, DecodeError::Custom("timebase is missing")),
      "{err:?}"
    );
  }
}

#[test]
fn timebase_wire_bytes_are_unchanged_by_the_signed_fields() {
  // Golden bytes captured from the `uint32` encoding this type used before
  // `num`/`den` became signed. `int32` and `uint32` are the same plain
  // varint for non-negative values, so the encoding must not have moved —
  // and old bytes must still decode to the same value.
  for (tb, golden) in [
    (
      Timebase::new(30_000, nz(1001)),
      &b"\x08\xb0\xea\x01\x10\xe9\x07"[..],
    ),
    (Timebase::new(0, nz(1)), &b"\x08\x00\x10\x01"[..]),
    (
      Timebase::new(1, nz(48_000)),
      &b"\x08\x01\x10\x80\xf7\x02"[..],
    ),
    (Timebase::new(1, nz(1)), &b"\x08\x01\x10\x01"[..]),
    (
      Timebase::new(i32::MAX, nz(i32::MAX)),
      &b"\x08\xff\xff\xff\xff\x07\x10\xff\xff\xff\xff\x07"[..],
    ),
  ] {
    assert_eq!(tb.encode_to_vec(), golden, "encoding moved for {tb:?}");
    assert_eq!(
      <Timebase as Message>::decode_from_slice(golden).expect("golden decodes"),
      tb
    );
  }
}

#[test]
fn timebase_unknown_field_is_skipped() {
  let mut buf: Vec<u8> = Vec::new();
  Tag::new(1, WireType::Varint).encode(&mut buf);
  encode_int32(2, &mut buf);
  Tag::new(2, WireType::Varint).encode(&mut buf);
  encode_int32(3, &mut buf);
  Tag::new(7, WireType::Varint).encode(&mut buf); // unknown field → skip_field_depth
  encode_varint(99, &mut buf);
  let tb = <Timebase as Message>::decode_from_slice(&buf).expect("unknown field skipped");
  assert_eq!(tb, Timebase::new(2, nz(3)));
}

// ---- TimeRange ----

#[test]
fn timerange_default_instance_and_clear() {
  let di = <TimeRange as DefaultInstance>::default_instance();
  assert_eq!((di.start_pts(), di.end_pts()), (0, 0));
  assert_eq!(di.timebase(), Timebase::default());
  let mut r = TimeRange::new(3, 5, Timebase::new(2, nz(3)));
  Message::clear(&mut r);
  assert_eq!((r.start_pts(), r.end_pts()), (0, 0));
  assert_eq!(r.timebase(), Timebase::default());
}

#[test]
fn timerange_wrong_wire_types_error() {
  let mut b1: Vec<u8> = Vec::new();
  Tag::new(1, WireType::LengthDelimited).encode(&mut b1);
  encode_varint(0, &mut b1);
  assert!(matches!(
    <TimeRange as Message>::decode_from_slice(&b1).unwrap_err(),
    DecodeError::WireTypeMismatch { field_number: 1, expected, actual }
      if expected == VARINT && actual == LEN
  ));
  let mut b2: Vec<u8> = Vec::new();
  Tag::new(2, WireType::LengthDelimited).encode(&mut b2);
  encode_varint(0, &mut b2);
  assert!(matches!(
    <TimeRange as Message>::decode_from_slice(&b2).unwrap_err(),
    DecodeError::WireTypeMismatch { field_number: 2, expected, actual }
      if expected == VARINT && actual == LEN
  ));
  let mut b3: Vec<u8> = Vec::new();
  Tag::new(3, WireType::Varint).encode(&mut b3);
  encode_varint(0, &mut b3);
  assert!(matches!(
    <TimeRange as Message>::decode_from_slice(&b3).unwrap_err(),
    DecodeError::WireTypeMismatch { field_number: 3, expected, actual }
      if expected == LEN && actual == VARINT
  ));
}

#[test]
fn timerange_unknown_field_is_skipped() {
  let original = TimeRange::new(10, 20, Timebase::new(30000, nz(1001)));
  let mut buf = original.encode_to_vec();
  Tag::new(9, WireType::Varint).encode(&mut buf); // unknown → skip_field_depth
  encode_varint(123, &mut buf);
  let r = <TimeRange as Message>::decode_from_slice(&buf).expect("unknown field skipped");
  assert_eq!(r, original);
}

// ---- Timestamp ----

#[test]
fn timestamp_default_instance_and_clear() {
  let di = <Timestamp as DefaultInstance>::default_instance();
  assert_eq!(di.pts(), 0);
  assert_eq!(di.timebase(), Timebase::default());
  let mut ts = Timestamp::new(42, Timebase::new(2, nz(3)));
  Message::clear(&mut ts);
  assert_eq!(ts.pts(), 0);
  assert_eq!(ts.timebase(), Timebase::default());
}

#[test]
fn timestamp_wrong_wire_types_error() {
  let mut b1: Vec<u8> = Vec::new();
  Tag::new(1, WireType::LengthDelimited).encode(&mut b1);
  encode_varint(0, &mut b1);
  assert!(matches!(
    <Timestamp as Message>::decode_from_slice(&b1).unwrap_err(),
    DecodeError::WireTypeMismatch { field_number: 1, expected, actual }
      if expected == VARINT && actual == LEN
  ));
  let mut b2: Vec<u8> = Vec::new();
  Tag::new(2, WireType::Varint).encode(&mut b2);
  encode_varint(0, &mut b2);
  assert!(matches!(
    <Timestamp as Message>::decode_from_slice(&b2).unwrap_err(),
    DecodeError::WireTypeMismatch { field_number: 2, expected, actual }
      if expected == LEN && actual == VARINT
  ));
}

#[test]
fn timestamp_unknown_field_is_skipped() {
  let original = Timestamp::new(-99, Timebase::new(24000, nz(1001)));
  let mut buf = original.encode_to_vec();
  Tag::new(6, WireType::Varint).encode(&mut buf); // unknown → skip_field_depth
  encode_varint(7, &mut buf);
  let ts = <Timestamp as Message>::decode_from_slice(&buf).expect("unknown field skipped");
  assert_eq!(ts, original);
}

// ---- TimeRange: the endpoint order ----

/// `start` and `end` as a peer might write them, in the order given, then a
/// `1/1` timebase.
fn endpoints(fields: &[(u32, i64)]) -> Vec<u8> {
  let mut buf = Vec::new();
  for &(field, value) in fields {
    Tag::new(field, WireType::Varint).encode(&mut buf);
    encode_int64(value, &mut buf);
  }
  let timebase = timebase_fields(&[(1, 1), (2, 1)]);
  Tag::new(3, WireType::LengthDelimited).encode(&mut buf);
  encode_varint(timebase.len() as u64, &mut buf);
  buf.extend_from_slice(&timebase);
  buf
}

fn is_inverted_refusal(err: &DecodeError) -> bool {
  matches!(err, DecodeError::Custom(reason) if *reason == INVERTED_RANGE)
}

#[test]
fn an_inverted_range_is_refused_at_decode() {
  // `end` before `start`, however the fields arrive — including an endpoint
  // alone that lands past the other's seeded zero.
  for fields in [
    &[(1, 100), (2, 50)][..],
    &[(2, 50), (1, 100)][..],
    &[(1, 5)][..],
    &[(2, -1)][..],
  ] {
    let err = <TimeRange as Message>::decode_from_slice(&endpoints(fields)).unwrap_err();
    assert!(is_inverted_refusal(&err), "{fields:?}: {err:?}");
  }
}

#[test]
fn a_range_may_pass_through_an_inverted_state_on_the_way() {
  // `start` first leaves `[100, 0)` for a moment; only the finished message
  // is judged.
  for fields in [&[(1, 100), (2, 200)][..], &[(2, 200), (1, 100)][..]] {
    let r = <TimeRange as Message>::decode_from_slice(&endpoints(fields)).expect("valid");
    assert_eq!((r.start_pts(), r.end_pts()), (100, 200), "{fields:?}");
  }
  // Equal endpoints are a valid, zero-length range.
  let r = <TimeRange as Message>::decode_from_slice(&endpoints(&[(1, 7), (2, 7)])).unwrap();
  assert!(r.is_instant());
}

#[test]
fn a_refused_merge_leaves_the_range_as_it_was() {
  let ms = Timebase::new(1, nz(1000));
  let before = TimeRange::new(0, 50, ms);

  // Merged into an existing value: `start = 100` alone would leave
  // `[100, 50)`.
  let mut r = before;
  let err = r.merge_from_slice(&endpoints(&[(1, 100)])).unwrap_err();
  assert!(is_inverted_refusal(&err));
  assert_eq!(r, before);

  // A merge that fails part-way leaves no inverted residue either.
  let mut truncated = endpoints(&[(1, 100)]);
  Tag::new(2, WireType::Varint).encode(&mut truncated);
  let mut r = before;
  assert!(matches!(
    r.merge_from_slice(&truncated),
    Err(DecodeError::UnexpectedEof | DecodeError::VarintTooLong)
  ));
  assert_eq!(r, before);

  // A read that is a whole, ordered range replaces the value: `{end: 200}`
  // with its timebase is `[0, 200) @ 1/1`, its absent start zero.
  let mut r = before;
  r.merge_from_slice(&endpoints(&[(2, 200)])).unwrap();
  assert_eq!(r, TimeRange::new(0, 200, Timebase::default()));
}

#[test]
fn every_decode_road_refuses_an_inverted_range() {
  let inverted = endpoints(&[(1, 100), (2, 50)]);
  let limit = core::cell::Cell::new(buffa::DEFAULT_UNKNOWN_FIELD_LIMIT);

  // Length-delimited, as a containing message decodes a field.
  let mut framed = Vec::new();
  encode_varint(inverted.len() as u64, &mut framed);
  framed.extend_from_slice(&inverted);
  let ctx = DecodeContext::new(buffa::RECURSION_LIMIT, &limit);
  let mut r = TimeRange::default();
  let err = r
    .merge_length_delimited(&mut framed.as_slice(), ctx)
    .unwrap_err();
  assert!(is_inverted_refusal(&err), "{err:?}");
  assert_eq!(r, TimeRange::default());
  assert!(is_inverted_refusal(
    &<TimeRange as Message>::decode_length_delimited(&mut framed.as_slice()).unwrap_err()
  ));

  // Group-encoded, as a delimited-encoding field is.
  let mut grouped = inverted.clone();
  Tag::new(7, WireType::EndGroup).encode(&mut grouped);
  let ctx = DecodeContext::new(buffa::RECURSION_LIMIT, &limit);
  let mut r = TimeRange::default();
  let err = r.merge_group(&mut grouped.as_slice(), ctx, 7).unwrap_err();
  assert!(is_inverted_refusal(&err), "{err:?}");
  assert_eq!(r, TimeRange::default());

  // And a well-ordered group still decodes, closed by its own field number.
  let mut ordered = endpoints(&[(1, 1), (2, 2)]);
  Tag::new(7, WireType::EndGroup).encode(&mut ordered);
  let ctx = DecodeContext::new(buffa::RECURSION_LIMIT, &limit);
  let mut r = TimeRange::default();
  r.merge_group(&mut ordered.as_slice(), ctx, 7).unwrap();
  assert_eq!((r.start_pts(), r.end_pts()), (1, 2));
  let ctx = DecodeContext::new(buffa::RECURSION_LIMIT, &limit);
  assert!(matches!(
    TimeRange::default().merge_group(&mut ordered.as_slice(), ctx, 8),
    Err(DecodeError::InvalidEndGroup(7))
  ));
}

#[test]
fn a_field_that_overruns_its_message_leaves_the_range_as_it_was() {
  // A one-byte message holding only the `start` tag: decoding its value
  // reads the next byte, which belongs to the enclosing buffer, as `5`. The
  // merge is refused for the overrun, and the range keeps its value.
  let before = TimeRange::new(0, 10, Timebase::new(1, nz(1000)));
  let framed = [0x01, 0x08, 0x05];
  let limit = core::cell::Cell::new(buffa::DEFAULT_UNKNOWN_FIELD_LIMIT);
  let ctx = DecodeContext::new(buffa::RECURSION_LIMIT, &limit);
  let mut r = before;
  assert!(matches!(
    r.merge_length_delimited(&mut framed.as_slice(), ctx),
    Err(DecodeError::UnexpectedEof)
  ));
  assert_eq!(r, before);

  // A whole message at the top level decodes, and replaces the value.
  let mut r = before;
  r.merge_from_slice(&endpoints(&[(1, 5), (2, 10)])).unwrap();
  assert_eq!(r, TimeRange::new(5, 10, Timebase::default()));
}
