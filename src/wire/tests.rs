use super::*;

fn nz(n: i32) -> NonZeroI32 {
  NonZeroI32::new(n).unwrap()
}

/// An enclosing message with one singular `TimeRange` field (number 1),
/// merged the way buffa's generated code merges a singular message field:
/// every occurrence into the same value, through `merge_length_delimited`.
/// Generic over the field's Rust type.
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
      timebase: Some(Timebase { num: 0, den: 3 })
    }
  );
  let range = TimeRange::decode_from_slice(&[0x10, 0x09, 0x1a, 0x02, 0x10, 0x03]).unwrap();
  assert_eq!(
    (range.start, range.end, range.timebase),
    (0, 9, Some(Timebase { num: 0, den: 3 }))
  );
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
      timebase: Some(read),
    };
    assert_eq!(crate::Timestamp::try_from(stamp), Err(refusal));
    let range = TimeRange {
      start: 1,
      end: 2,
      timebase: Some(read),
    };
    assert_eq!(crate::TimeRange::try_from(range), Err(refusal));
  }

  // A timebase message that is present but empty has a zero denominator.
  let range = TimeRange::decode_from_slice(&[0x10, 0x09, 0x1a, 0x00]).unwrap();
  assert_eq!(range.timebase, Some(Timebase { num: 0, den: 0 }));
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
  }
  let present = timebases.map(Some);
  for timebase in present.into_iter().chain([None]) {
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
fn a_domain_value_round_trips_through_the_wire() {
  // Domain → wire → bytes → wire → domain, as written, for each type.
  let tb = crate::Timebase::new(30_000, nz(1001));
  let zero = crate::Timebase::new(0, nz(1));
  for range in [
    crate::TimeRange::new(0, 0, crate::Timebase::default()),
    crate::TimeRange::new(100, 250, tb),
    crate::TimeRange::new(-5, 0, tb),
    crate::TimeRange::new(i64::MIN, i64::MAX, zero),
  ] {
    let read = TimeRange::decode_from_slice(&TimeRange::from(range).encode_to_vec()).unwrap();
    assert_eq!(read, TimeRange::from(range), "{range:?}");
    assert_eq!(crate::TimeRange::try_from(read), Ok(range));
  }
  for stamp in [
    crate::Timestamp::new(0, crate::Timebase::MILLIS),
    crate::Timestamp::new(-90_000, crate::Timebase::MPEG_90K),
    crate::Timestamp::new(i64::MAX, zero),
  ] {
    let read = Timestamp::decode_from_slice(&Timestamp::from(stamp).encode_to_vec()).unwrap();
    assert_eq!(read, Timestamp::from(stamp), "{stamp:?}");
    let back = crate::Timestamp::try_from(read).unwrap();
    assert_eq!(Timestamp::from(back), Timestamp::from(stamp), "as written");
  }
  for timebase in [tb, zero, crate::Timebase::new(i32::MAX, nz(i32::MAX))] {
    let read = Timebase::decode_from_slice(&Timebase::from(timebase).encode_to_vec()).unwrap();
    let back = crate::Timebase::try_from(read).unwrap();
    assert_eq!(Timebase::from(back), Timebase::from(timebase), "as written");
  }
}

#[test]
fn a_conversion_error_says_what_failed() {
  for (error, message) in [
    (ConversionError::MissingTimebase, "timebase is missing"),
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
    timebase: Some(Timebase { num: 1, den: 1000 }),
  };
  assert_eq!(
    crate::TimeRange::try_from(wire),
    Ok(crate::TimeRange::new(7, 7, crate::Timebase::MILLIS)),
    "equal endpoints are a valid, zero-length range"
  );
}

#[test]
fn a_message_without_a_timebase_re_encodes_without_one() {
  // Empty in, empty out — byte for byte — for both messages that nest one.
  let stamp = Timestamp::decode_from_slice(&[]).unwrap();
  assert_eq!(
    stamp,
    Timestamp {
      pts: 0,
      timebase: None
    }
  );
  assert!(stamp.encode_to_vec().is_empty());
  let range = TimeRange::decode_from_slice(&[]).unwrap();
  assert_eq!(range.timebase, None);
  assert!(range.encode_to_vec().is_empty());

  // A count alone stays a count alone.
  let bytes = [0x08, 0x05];
  let stamp = Timestamp::decode_from_slice(&bytes).unwrap();
  assert_eq!(stamp.encode_to_vec(), bytes);
}

#[test]
fn merging_a_message_without_a_timebase_keeps_the_one_there() {
  let tb = Timebase {
    num: 1,
    den: 90_000,
  };
  let empty = Timestamp::decode_from_slice(&[]).unwrap().encode_to_vec();
  let mut stamp = Timestamp {
    pts: 5,
    timebase: Some(tb),
  };
  stamp.merge_from_slice(&empty).unwrap();
  assert_eq!(stamp.timebase, Some(tb));

  let empty = TimeRange::decode_from_slice(&[]).unwrap().encode_to_vec();
  let mut range = TimeRange {
    start: 1,
    end: 2,
    timebase: Some(tb),
  };
  range.merge_from_slice(&empty).unwrap();
  assert_eq!(range.timebase, Some(tb));
}

#[test]
fn an_absent_timebase_is_refused_at_conversion_by_name() {
  let stamp = Timestamp {
    pts: 1,
    timebase: None,
  };
  assert_eq!(
    crate::Timestamp::try_from(stamp),
    Err(ConversionError::MissingTimebase)
  );
  let range = TimeRange::decode_from_slice(&range_body(&[(2, 9)], None)).unwrap();
  assert_eq!(range.timebase, None);
  assert_eq!(
    crate::TimeRange::try_from(range),
    Err(ConversionError::MissingTimebase)
  );
}

quickcheck::quickcheck! {
  /// Over every range the domain type can hold, the two conversions are
  /// inverses, and the wire codec reads back what it writes — so this
  /// version never writes a range it cannot read.
  fn the_conversions_are_inverses_over_every_range(a: i64, b: i64, num: u32, den: u32) -> bool {
    let timebase = crate::Timebase::new(
      (num % (i32::MAX as u32 + 1)) as i32,
      nz((den % i32::MAX as u32 + 1) as i32),
    );
    let range = crate::TimeRange::new(a.min(b), a.max(b), timebase);
    let wire = TimeRange::from(range);
    let decoded = TimeRange::decode_from_slice(&wire.encode_to_vec());
    crate::TimeRange::try_from(wire).map(TimeRange::from) == Ok(wire) && decoded.ok() == Some(wire)
  }
}

#[test]
fn a_view_keeps_the_callers_decode_limits() {
  // A timestamp whose timebase field is one nesting level down: no depth to
  // spare refuses it, through the view as through the message.
  let bytes = [0x12, 0x00];
  let none = buffa::DecodeOptions::new().with_recursion_limit(0);
  assert!(matches!(
    none.decode_view::<Timestamp>(&bytes),
    Err(DecodeError::RecursionLimitExceeded)
  ));
  assert!(matches!(
    none.decode::<Timestamp>(&mut bytes.as_slice()),
    Err(DecodeError::RecursionLimitExceeded)
  ));
  assert!(matches!(
    none.decode_view::<TimeRange>(&[0x1a, 0x00]),
    Err(DecodeError::RecursionLimitExceeded)
  ));

  // One level is enough.
  let one = buffa::DecodeOptions::new().with_recursion_limit(1);
  assert_eq!(
    one.decode_view::<Timestamp>(&bytes).ok(),
    Some(Timestamp {
      pts: 0,
      timebase: Some(Timebase::default()),
    })
  );
}

quickcheck::quickcheck! {
  /// Every timebase and timestamp the domain types can hold round-trips
  /// through the wire — a zero numerator included, which the small draw
  /// reaches often.
  fn every_timebase_and_timestamp_round_trips_through_the_wire(pts: i64, num: u32, den: u32, small: bool) -> bool {
    let num = if small { num % 3 } else { num % (i32::MAX as u32 + 1) };
    let timebase = crate::Timebase::new(num as i32, nz((den % i32::MAX as u32 + 1) as i32));
    let stamp = crate::Timestamp::new(pts, timebase);
    let read_timebase = Timebase::decode_from_slice(&Timebase::from(timebase).encode_to_vec());
    let read_stamp = Timestamp::decode_from_slice(&Timestamp::from(stamp).encode_to_vec());
    let timebase_back = read_timebase.ok().and_then(|t| crate::Timebase::try_from(t).ok());
    let stamp_back = read_stamp.ok().and_then(|t| crate::Timestamp::try_from(t).ok());
    timebase_back.map(Timebase::from) == Some(Timebase::from(timebase))
      && stamp_back.map(Timestamp::from) == Some(Timestamp::from(stamp))
  }
}

// ---- The codec itself: defaults, wire types, unknown fields, old bytes ----

#[test]
fn the_default_instances_are_the_zero_messages_and_clear_returns_to_them() {
  assert_eq!(*Timebase::default_instance(), Timebase { num: 0, den: 0 });
  assert_eq!(*Timestamp::default_instance(), Timestamp::default());
  assert_eq!(*TimeRange::default_instance(), TimeRange::default());
  let mut timebase = Timebase { num: 7, den: 9 };
  Message::clear(&mut timebase);
  assert_eq!(timebase, Timebase::default());
  let mut stamp = Timestamp {
    pts: 3,
    timebase: Some(timebase),
  };
  Message::clear(&mut stamp);
  assert_eq!(stamp, Timestamp::default());
  let mut range = TimeRange {
    start: 1,
    end: 2,
    timebase: Some(timebase),
  };
  Message::clear(&mut range);
  assert_eq!(range, TimeRange::default());
}

#[test]
fn a_field_of_the_wrong_wire_type_is_refused() {
  let varint = WireType::Varint as u8;
  let len = WireType::LengthDelimited as u8;
  let mismatch = |field: u32, wire_type: WireType| {
    let mut buf = Vec::new();
    Tag::new(field, wire_type).encode(&mut buf);
    encode_varint(0, &mut buf);
    buf
  };
  for (err, field, expected) in [
    (
      Timebase::decode_from_slice(&mismatch(1, WireType::LengthDelimited)).unwrap_err(),
      1,
      varint,
    ),
    (
      Timebase::decode_from_slice(&mismatch(2, WireType::LengthDelimited)).unwrap_err(),
      2,
      varint,
    ),
    (
      Timestamp::decode_from_slice(&mismatch(1, WireType::LengthDelimited)).unwrap_err(),
      1,
      varint,
    ),
    (
      Timestamp::decode_from_slice(&mismatch(2, WireType::Varint)).unwrap_err(),
      2,
      len,
    ),
    (
      TimeRange::decode_from_slice(&mismatch(1, WireType::LengthDelimited)).unwrap_err(),
      1,
      varint,
    ),
    (
      TimeRange::decode_from_slice(&mismatch(2, WireType::LengthDelimited)).unwrap_err(),
      2,
      varint,
    ),
    (
      TimeRange::decode_from_slice(&mismatch(3, WireType::Varint)).unwrap_err(),
      3,
      len,
    ),
  ] {
    assert!(
      matches!(err, DecodeError::WireTypeMismatch { field_number, expected: e, .. }
        if field_number == field && e == expected),
      "field {field}: {err:?}"
    );
  }
}

#[test]
fn an_unknown_field_is_skipped() {
  let unknown = |mut buf: Vec<u8>| {
    Tag::new(9, WireType::Varint).encode(&mut buf);
    encode_varint(123, &mut buf);
    buf
  };
  let timebase = Timebase { num: 2, den: 3 };
  assert_eq!(
    Timebase::decode_from_slice(&unknown(timebase.encode_to_vec())).ok(),
    Some(timebase)
  );
  let stamp = Timestamp {
    pts: 5,
    timebase: Some(timebase),
  };
  assert_eq!(
    Timestamp::decode_from_slice(&unknown(stamp.encode_to_vec())).ok(),
    Some(stamp)
  );
  let range = TimeRange {
    start: 10,
    end: 20,
    timebase: Some(timebase),
  };
  assert_eq!(
    TimeRange::decode_from_slice(&unknown(range.encode_to_vec())).ok(),
    Some(range)
  );
}

#[test]
fn the_bytes_earlier_versions_wrote_still_read() {
  // Golden bytes from the `uint32` encoding 0.3 wrote, and the explicit zero
  // numerator 0.4 wrote: `int32` and `uint32` are one varint for these
  // values, and an explicit zero reads as the zero it is. Only the zero
  // numerator's own encoding moved, to proto3's elided form.
  for (num, den, golden, now) in [
    (
      30_000,
      1001,
      &b"\x08\xb0\xea\x01\x10\xe9\x07"[..],
      &b"\x08\xb0\xea\x01\x10\xe9\x07"[..],
    ),
    (0, 1, &b"\x08\x00\x10\x01"[..], &b"\x10\x01"[..]),
    (
      1,
      48_000,
      &b"\x08\x01\x10\x80\xf7\x02"[..],
      &b"\x08\x01\x10\x80\xf7\x02"[..],
    ),
    (
      i32::MAX,
      i32::MAX,
      &b"\x08\xff\xff\xff\xff\x07\x10\xff\xff\xff\xff\x07"[..],
      &b"\x08\xff\xff\xff\xff\x07\x10\xff\xff\xff\xff\x07"[..],
    ),
  ] {
    let read = Timebase::decode_from_slice(golden).unwrap();
    assert_eq!(read, Timebase { num, den });
    assert_eq!(read.encode_to_vec(), now, "{num}/{den}");
  }
}
