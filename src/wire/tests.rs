use super::*;

use core::num::NonZeroI32;

fn nz(n: i32) -> NonZeroI32 {
  NonZeroI32::new(n).unwrap()
}

/// An enclosing message with one singular `TimeRange` field (number 1),
/// merged the way buffa's generated code merges a singular message field:
/// every occurrence into the same value, through `merge_length_delimited`.
/// Generic over the field's Rust type, so the same bytes can be decoded
/// through either mapping.
#[derive(Debug, Default, Clone, PartialEq)]
struct Clip<R> {
  range: R,
}

impl<R: Message + Copy> DefaultInstance for Clip<R> {
  fn default_instance() -> &'static Self {
    unreachable!("never asked for by these tests")
  }
}

impl<R: Message + Copy> Message for Clip<R> {
  fn compute_size(&self, _cache: &mut SizeCache) -> u32 {
    unreachable!("these tests only decode")
  }

  fn write_to(&self, _cache: &mut SizeCache, _buf: &mut impl EncodeSink) {
    unreachable!("these tests only decode")
  }

  fn merge_field(
    &mut self,
    tag: Tag,
    buf: &mut impl Buf,
    ctx: DecodeContext<'_>,
  ) -> Result<(), DecodeError> {
    match tag.field_number() {
      1 => Message::merge_length_delimited(&mut self.range, buf, ctx),
      _ => skip_field_depth(tag, buf, ctx.depth()),
    }
  }

  fn clear(&mut self) {
    *self = Self::default();
  }
}

/// One occurrence of field 1 of the enclosing message, carrying the given
/// endpoint fields of a range.
fn occurrence(out: &mut Vec<u8>, fields: &[(u32, i64)]) {
  let mut body = Vec::new();
  for &(field, value) in fields {
    Tag::new(field, WireType::Varint).encode(&mut body);
    encode_int64(value, &mut body);
  }
  Tag::new(1, WireType::LengthDelimited).encode(out);
  encode_varint(body.len() as u64, out);
  out.extend_from_slice(&body);
}

#[test]
fn a_range_split_over_two_occurrences_merges_into_one() {
  // `{start: 100}`, then `{end: 200}`: protobuf merges them into
  // `[100, 200)`, and the first occurrence alone is inverted.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &[(1, 100)]);
  occurrence(&mut bytes, &[(2, 200)]);

  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).expect("a split field decodes");
  assert_eq!((clip.range.start, clip.range.end), (100, 200));
  assert_eq!(
    crate::TimeRange::try_from(clip.range),
    Ok(crate::TimeRange::new(100, 200, Timebase::default()))
  );

  // In either order, and with the second occurrence overriding the first.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &[(2, 200)]);
  occurrence(&mut bytes, &[(1, 300), (2, 50)]);
  occurrence(&mut bytes, &[(1, 10)]);
  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).unwrap();
  assert_eq!((clip.range.start, clip.range.end), (10, 50));
}

#[test]
fn an_inverted_final_value_still_refuses() {
  // One occurrence, `{start: 100}` over the seeded `end = 0`: the field is
  // read as written, and the conversion refuses it.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &[(1, 100)]);
  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).unwrap();
  assert_eq!((clip.range.start, clip.range.end), (100, 0));
  assert_eq!(
    crate::TimeRange::try_from(clip.range),
    Err(InvertedRange(()))
  );

  // A split whose final value is inverted refuses the same way.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &[(1, 100), (2, 200)]);
  occurrence(&mut bytes, &[(2, 50)]);
  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).unwrap();
  assert!(crate::TimeRange::try_from(clip.range).is_err());
}

#[test]
fn the_domain_mapping_judges_each_occurrence() {
  // Decoded straight into the domain type, the same split is refused at its
  // first occurrence — the documented cost of a mapping that never holds an
  // inverted range — while a field in one occurrence decodes.
  let mut split = Vec::new();
  occurrence(&mut split, &[(1, 100)]);
  occurrence(&mut split, &[(2, 200)]);
  assert!(matches!(
    Clip::<crate::TimeRange>::decode_from_slice(&split),
    Err(DecodeError::Custom(_))
  ));

  let mut whole = Vec::new();
  occurrence(&mut whole, &[(1, 100), (2, 200)]);
  let clip = Clip::<crate::TimeRange>::decode_from_slice(&whole).unwrap();
  assert_eq!(
    clip.range,
    crate::TimeRange::new(100, 200, Timebase::default())
  );
}

#[test]
fn the_two_mappings_share_one_encoding() {
  let tb = Timebase::new(30_000, nz(1001));
  for range in [
    crate::TimeRange::new(0, 0, Timebase::default()),
    crate::TimeRange::new(100, 250, tb),
    crate::TimeRange::new(-5, 0, tb),
    crate::TimeRange::new(i64::MIN, i64::MAX, Timebase::new(0, nz(1))),
  ] {
    let domain = range.encode_to_vec();
    let wire = TimeRange::from(range).encode_to_vec();
    assert_eq!(domain, wire, "{range:?}");
    let read = TimeRange::decode_from_slice(&domain).unwrap();
    assert_eq!(crate::TimeRange::try_from(read), Ok(range));
    assert_eq!(
      <crate::TimeRange as Message>::decode_from_slice(&wire).ok(),
      Some(range)
    );
  }
}

#[test]
fn an_inverted_range_says_what_failed() {
  assert_eq!(
    format!("{}", InvertedRange(())),
    "time range end precedes its start"
  );
  let wire = TimeRange {
    start: 7,
    end: 7,
    timebase: Timebase::MILLIS,
  };
  assert_eq!(
    crate::TimeRange::try_from(wire),
    Ok(crate::TimeRange::new(7, 7, Timebase::MILLIS)),
    "equal endpoints are a valid, zero-length range"
  );
}
