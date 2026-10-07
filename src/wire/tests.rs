use super::*;

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

/// A range message's body: the given endpoint fields, and the timebase
/// `num/den` when one is given.
fn range_body(fields: &[(u32, i64)], timebase: Option<(i32, i32)>) -> Vec<u8> {
  let mut body = Vec::new();
  for &(field, value) in fields {
    Tag::new(field, WireType::Varint).encode(&mut body);
    encode_int64(value, &mut body);
  }
  if let Some((num, den)) = timebase {
    let inner = Timebase { num, den }.encode_to_vec();
    Tag::new(3, WireType::LengthDelimited).encode(&mut body);
    encode_varint(inner.len() as u64, &mut body);
    body.extend_from_slice(&inner);
  }
  body
}

/// One occurrence of field 1 of the enclosing message.
fn occurrence(out: &mut Vec<u8>, body: &[u8]) {
  Tag::new(1, WireType::LengthDelimited).encode(out);
  encode_varint(body.len() as u64, out);
  out.extend_from_slice(body);
}

const MS: Option<(i32, i32)> = Some((1, 1000));

#[test]
fn a_range_split_over_two_occurrences_merges_into_one() {
  // `{start: 100, timebase: 1/1000}`, then `{end: 200}`: protobuf merges them
  // into `[100, 200) @ 1/1000`, and the first occurrence alone is inverted.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &range_body(&[(1, 100)], MS));
  occurrence(&mut bytes, &range_body(&[(2, 200)], None));

  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).expect("a split field decodes");
  assert_eq!((clip.range.start, clip.range.end), (100, 200));
  assert_eq!(
    crate::TimeRange::try_from(clip.range),
    Ok(crate::TimeRange::new(100, 200, crate::Timebase::MILLIS))
  );

  // In either order, and with a later occurrence overriding an earlier one.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &range_body(&[(2, 200)], MS));
  occurrence(&mut bytes, &range_body(&[(1, 300), (2, 50)], None));
  occurrence(&mut bytes, &range_body(&[(1, 10)], None));
  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).unwrap();
  assert_eq!((clip.range.start, clip.range.end), (10, 50));
}

#[test]
fn an_inverted_final_value_still_refuses() {
  // One occurrence, `{start: 100}` over protobuf's zero `end`: the field is
  // read as written, and the conversion refuses it by name.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &range_body(&[(1, 100)], MS));
  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).unwrap();
  assert_eq!((clip.range.start, clip.range.end), (100, 0));
  assert_eq!(
    crate::TimeRange::try_from(clip.range),
    Err(ConversionError::InvertedRange)
  );

  // A split whose final value is inverted refuses the same way.
  let mut bytes = Vec::new();
  occurrence(&mut bytes, &range_body(&[(1, 100), (2, 200)], MS));
  occurrence(&mut bytes, &range_body(&[(2, 50)], None));
  let clip = Clip::<TimeRange>::decode_from_slice(&bytes).unwrap();
  assert_eq!(
    crate::TimeRange::try_from(clip.range),
    Err(ConversionError::InvertedRange)
  );
}

#[test]
fn the_domain_mapping_judges_each_occurrence() {
  // Decoded straight into the domain type, the same split is refused at its
  // first occurrence — the documented cost of a mapping that never holds an
  // inverted range — while a field in one occurrence decodes.
  let mut split = Vec::new();
  occurrence(&mut split, &range_body(&[(1, 100)], MS));
  occurrence(&mut split, &range_body(&[(2, 200)], None));
  assert!(matches!(
    Clip::<crate::TimeRange>::decode_from_slice(&split),
    Err(DecodeError::Custom(_))
  ));

  let mut whole = Vec::new();
  occurrence(&mut whole, &range_body(&[(1, 100), (2, 200)], MS));
  let clip = Clip::<crate::TimeRange>::decode_from_slice(&whole).unwrap();
  assert_eq!(
    clip.range,
    crate::TimeRange::new(100, 200, crate::Timebase::MILLIS)
  );
}

#[test]
fn a_zero_numerator_elided_by_a_proto3_encoder_reads_back_as_zero() {
  // proto3 writes `0/3` as the denominator alone: `[0x10, 0x03]`.
  let timebase = Timebase::decode_from_slice(&[0x10, 0x03]).unwrap();
  assert_eq!(timebase, Timebase { num: 0, den: 3 });
  let domain = crate::Timebase::try_from(timebase).unwrap();
  assert_eq!((domain.num(), domain.den().get()), (0, 3));

  // Nested, as a timestamp's and a range's timebase field.
  let stamp = Timestamp::decode_from_slice(&[0x08, 0x05, 0x12, 0x02, 0x10, 0x03]).unwrap();
  assert_eq!(
    stamp,
    Timestamp {
      pts: 5,
      timebase: Timebase { num: 0, den: 3 }
    }
  );
  let range = TimeRange::decode_from_slice(&[0x10, 0x09, 0x1a, 0x02, 0x10, 0x03]).unwrap();
  assert_eq!(
    (range.start, range.end, range.timebase),
    (0, 9, Timebase { num: 0, den: 3 })
  );

  // The domain mapping starts from `1/1`, and reads the same bytes as `1/3`.
  let misread = <crate::Timebase as Message>::decode_from_slice(&[0x10, 0x03]).unwrap();
  assert_eq!((misread.num(), misread.den().get()), (1, 3));
}

#[test]
fn a_bad_raw_timebase_refuses_at_conversion_by_name_and_is_never_clamped() {
  for (num, den, refusal) in [
    (1, 0, ConversionError::ZeroDenominator),
    (1, -5, ConversionError::NegativeDenominator),
    (-1, 1000, ConversionError::NegativeNumerator),
    (-1, 0, ConversionError::NegativeNumerator),
  ] {
    let bytes = Timebase { num, den }.encode_to_vec();
    let read = Timebase::decode_from_slice(&bytes).unwrap();
    assert_eq!(read, Timebase { num, den }, "kept as written");
    assert_eq!(crate::Timebase::try_from(read), Err(refusal), "{num}/{den}");
    let stamp = Timestamp {
      pts: 1,
      timebase: read,
    };
    assert_eq!(crate::Timestamp::try_from(stamp), Err(refusal));
    let range = TimeRange {
      start: 1,
      end: 2,
      timebase: read,
    };
    assert_eq!(crate::TimeRange::try_from(range), Err(refusal));
  }

  // An absent timebase is protobuf's zero message, and has no denominator.
  let range = TimeRange::decode_from_slice(&range_body(&[(2, 9)], None)).unwrap();
  assert_eq!(range.timebase, Timebase { num: 0, den: 0 });
  assert_eq!(
    crate::TimeRange::try_from(range),
    Err(ConversionError::ZeroDenominator)
  );

  // A zero numerator is legal: the degenerate timebase.
  let zero = crate::Timebase::try_from(Timebase { num: 0, den: 7 }).unwrap();
  assert_eq!((zero.num(), zero.den().get()), (0, 7));
}

#[test]
fn every_field_round_trips_as_written() {
  let timebases = [
    Timebase { num: 0, den: 0 },
    Timebase {
      num: i32::MIN,
      den: i32::MAX,
    },
    Timebase {
      num: 30_000,
      den: 1001,
    },
    Timebase { num: 7, den: -3 },
  ];
  for timebase in timebases {
    let read = Timebase::decode_from_slice(&timebase.encode_to_vec()).ok();
    assert_eq!(read, Some(timebase));
    for pts in [0, -1, i64::MIN, i64::MAX] {
      let stamp = Timestamp { pts, timebase };
      let read = Timestamp::decode_from_slice(&stamp.encode_to_vec()).ok();
      assert_eq!(read, Some(stamp));
      let range = TimeRange {
        start: pts,
        end: pts.wrapping_neg(),
        timebase,
      };
      let read = TimeRange::decode_from_slice(&range.encode_to_vec()).ok();
      assert_eq!(read, Some(range));
    }
  }
}

#[test]
fn the_two_mappings_share_one_encoding() {
  let tb = crate::Timebase::new(30_000, nz(1001));
  let zero = crate::Timebase::new(0, nz(1));
  for range in [
    crate::TimeRange::new(0, 0, crate::Timebase::default()),
    crate::TimeRange::new(100, 250, tb),
    crate::TimeRange::new(-5, 0, tb),
    crate::TimeRange::new(i64::MIN, i64::MAX, zero),
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
  for stamp in [
    crate::Timestamp::new(0, crate::Timebase::MILLIS),
    crate::Timestamp::new(-90_000, crate::Timebase::MPEG_90K),
    crate::Timestamp::new(i64::MAX, zero),
  ] {
    let domain = stamp.encode_to_vec();
    assert_eq!(domain, Timestamp::from(stamp).encode_to_vec());
    let read = Timestamp::decode_from_slice(&domain).unwrap();
    let back = crate::Timestamp::try_from(read).unwrap();
    assert_eq!(Timestamp::from(back), Timestamp::from(stamp), "as written");
  }
  for timebase in [tb, zero, crate::Timebase::new(i32::MAX, nz(i32::MAX))] {
    let domain = timebase.encode_to_vec();
    assert_eq!(domain, Timebase::from(timebase).encode_to_vec());
    let back = crate::Timebase::try_from(Timebase::decode_from_slice(&domain).unwrap()).unwrap();
    assert_eq!(Timebase::from(back), Timebase::from(timebase), "as written");
  }
}

#[test]
fn a_conversion_error_says_what_failed() {
  for (error, message) in [
    (
      ConversionError::NegativeNumerator,
      "timebase numerator is negative",
    ),
    (
      ConversionError::ZeroDenominator,
      "timebase denominator is zero",
    ),
    (
      ConversionError::NegativeDenominator,
      "timebase denominator is negative",
    ),
    (
      ConversionError::InvertedRange,
      "time range end precedes its start",
    ),
  ] {
    assert_eq!(format!("{error}"), message);
  }
  let wire = TimeRange {
    start: 7,
    end: 7,
    timebase: Timebase { num: 1, den: 1000 },
  };
  assert_eq!(
    crate::TimeRange::try_from(wire),
    Ok(crate::TimeRange::new(7, 7, crate::Timebase::MILLIS)),
    "equal endpoints are a valid, zero-length range"
  );
}
