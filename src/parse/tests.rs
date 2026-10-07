use super::*;

use crate::{WELL_KNOWN, WELL_KNOWN_RATES, nz};

/// The derived `Debug` prints every field, which makes it the *structural*
/// comparison this crate's `PartialEq` deliberately is not: `2/4 == 1/2`
/// under `==`, so `==` alone would not catch a parser that silently reduced.
fn assert_same_fields<T: fmt::Debug>(parsed: &T, original: &T) {
  assert_eq!(format!("{parsed:?}"), format!("{original:?}"));
}

fn timebases() -> [Timebase; 7] {
  [
    Timebase::new(1, nz(1000)),
    Timebase::new(1, nz(90_000)),
    Timebase::new(30_000, nz(1001)),
    Timebase::new(0, nz(3)),
    Timebase::new(2, nz(4)),
    Timebase::default(),
    Timebase::new(i32::MAX, nz(i32::MAX)),
  ]
}

#[test]
fn timebase_round_trips_through_display() {
  for tb in timebases() {
    // `{}` and `{:#}` are the same rendering for this type, so both are
    // inverses.
    for rendered in [format!("{tb}"), format!("{tb:#}")] {
      let parsed: Timebase = rendered.parse().expect("Display output parses back");
      assert_eq!(parsed, tb);
      assert_same_fields(&parsed, &tb);
    }
  }
}

#[test]
fn timebase_parse_does_not_reduce() {
  // `Display` writes the declared form rather than the reduced one, so the
  // inverse must keep the declared form too — a parser that reduced would
  // still satisfy `==`.
  let parsed: Timebase = "2/4".parse().expect("parses");
  assert_eq!(parsed.num(), 2);
  assert_eq!(parsed.den().get(), 4);
}

#[test]
fn timebase_parse_trims_whitespace() {
  for s in ["1/1000", " 1/1000", "1/1000 ", "1 / 1000", "  1  /  1000  "] {
    assert_same_fields(
      &s.parse::<Timebase>().expect("parses"),
      &Timebase::new(1, nz(1000)),
    );
  }
}

#[test]
fn timebase_parse_rejects_what_the_constructor_rejects() {
  // The constructor's sign invariants are what the arithmetic assumes, so
  // parsing must not be a second way in.
  for s in ["-1/1000", "1/0", "1/-1000", "-1/-1"] {
    assert_eq!(s.parse::<Timebase>(), Err(ParseTimebaseError(())), "{s}");
  }
}

#[test]
fn timebase_parses_a_well_known_name_on_its_other_arm() {
  // The roster arm is tried first; `Display` is unchanged, so the round trip
  // it inverts still goes through `num/den`.
  for (name, expected) in WELL_KNOWN {
    assert_same_fields(&name.parse::<Timebase>().expect("parses"), expected);
    assert_eq!(
      format!("{expected}"),
      format!("{}/{}", expected.num(), expected.den())
    );
  }

  // Whitespace is trimmed around a name as it is around a rational.
  assert_same_fields(
    &"  MPEG_90K  ".parse::<Timebase>().expect("parses"),
    &Timebase::MPEG_90K,
  );

  // The name is an input convenience only: two spellings, one value, and the
  // rendering is always the rational.
  assert_eq!("MILLIS".parse::<Timebase>(), "1/1000".parse::<Timebase>());

  // The composite parsers inherit the arm, since they parse their timebase
  // half through this impl.
  assert_same_fields(
    &"12345 @ MPEG_90K".parse::<Timestamp>().expect("parses"),
    &Timestamp::new(12_345, Timebase::MPEG_90K),
  );
  assert_same_fields(
    &"[100, 500) @ MILLIS".parse::<TimeRange>().expect("parses"),
    &TimeRange::new(100, 500, Timebase::MILLIS),
  );
}

#[test]
fn timebase_parse_rejects_a_name_that_is_not_on_the_roster() {
  // Case folds, per `Timebase::from_name`, and nothing else does; a near
  // miss falls through to the rational arm, which has no slash to find.
  for s in ["MILLI", "MPEG90K", "SECOND", "NTSC", "milli_s"] {
    assert_eq!(s.parse::<Timebase>(), Err(ParseTimebaseError(())), "{s}");
  }
}

#[test]
fn timebase_parse_folds_case_on_the_name_arm() {
  // The door is `Timebase::from_name`, so `FromStr` inherits its folding —
  // and so do the composite parsers, which parse their timebase half here.
  for s in ["millis", "Millis", "  mIlLiS  "] {
    assert_same_fields(&s.parse::<Timebase>().expect("parses"), &Timebase::MILLIS);
  }
  assert_same_fields(
    &"12345 @ mpeg_90k".parse::<Timestamp>().expect("parses"),
    &Timestamp::new(12_345, Timebase::MPEG_90K),
  );
  assert_same_fields(
    &"[100, 500) @ millis".parse::<TimeRange>().expect("parses"),
    &TimeRange::new(100, 500, Timebase::MILLIS),
  );
}

#[test]
fn timebase_parse_rejects_malformed_input() {
  for s in [
    "",
    "1",
    "/1000",
    "1/",
    "1/2/3",
    "a/b",
    "1.5/2",
    "1/1000 @ 2",
    // Beyond `i32` at either end.
    "2147483648/1",
    "1/2147483648",
  ] {
    assert_eq!(s.parse::<Timebase>(), Err(ParseTimebaseError(())), "{s}");
  }
}

#[test]
fn timestamp_round_trips_through_the_exact_display() {
  for tb in timebases() {
    for pts in [0i64, 1, -1, 12_345, -1500, i64::MAX, i64::MIN] {
      let ts = Timestamp::new(pts, tb);
      let parsed: Timestamp = format!("{ts:#}").parse().expect("`{:#}` parses back");
      assert_eq!(parsed, ts);
      assert_same_fields(&parsed, &ts);
    }
  }
}

#[test]
fn timestamp_parse_rejects_the_clock_form() {
  // The clock is truncated to milliseconds and names no timebase, so it
  // cannot name back the instant it was printed from. Two timestamps that
  // are *not* equal share this rendering.
  let a = Timestamp::new(12_345, Timebase::new(1, nz(90_000)));
  let b = Timestamp::new(137, Timebase::new(1, nz(1000)));
  assert_eq!(format!("{a}"), format!("{b}"));
  assert_ne!(a, b);
  assert_eq!(
    format!("{a}").parse::<Timestamp>(),
    Err(ParseTimestampError(()))
  );
}

#[test]
fn timestamp_parse_trims_whitespace_around_the_separator() {
  let expected = Timestamp::new(12_345, Timebase::new(1, nz(90_000)));
  for s in ["12345 @ 1/90000", "12345@1/90000", "  12345  @  1/90000  "] {
    assert_same_fields(&s.parse::<Timestamp>().expect("parses"), &expected);
  }
}

#[test]
fn timestamp_parse_rejects_malformed_input() {
  for s in [
    "",
    "12345",
    "@1/1000",
    "12345 @",
    "12345 @ 1/1000 @ 2",
    "abc @ 1/1000",
    "12345 @ -1/1000",
    "0:00:00.137",
  ] {
    assert_eq!(s.parse::<Timestamp>(), Err(ParseTimestampError(())), "{s}");
  }
}

#[test]
fn time_range_round_trips_through_the_exact_display() {
  let ms = Timebase::new(1, nz(1000));
  for range in [
    TimeRange::new(1500, 3250, ms),
    TimeRange::new(-1500, 3250, ms),
    TimeRange::new(0, 0, Timebase::default()),
    TimeRange::new(i64::MIN, i64::MAX, Timebase::new(30_000, nz(1001))),
    TimeRange::instant(Timestamp::new(12_345, Timebase::new(1, nz(90_000)))),
  ] {
    let parsed: TimeRange = format!("{range:#}").parse().expect("`{:#}` parses back");
    assert_eq!(parsed, range);
    assert_same_fields(&parsed, &range);
  }
}

#[test]
fn time_range_parse_rejects_the_clock_form() {
  let range = TimeRange::new(1500, 3250, Timebase::new(1, nz(1000)));
  assert_eq!(
    format!("{range}").parse::<TimeRange>(),
    Err(ParseTimeRangeError(()))
  );
}

#[test]
fn time_range_parse_trims_whitespace() {
  let expected = TimeRange::new(1500, 3250, Timebase::new(1, nz(1000)));
  for s in [
    "[1500, 3250) @ 1/1000",
    "[1500,3250)@1/1000",
    "  [ 1500 , 3250 ) @ 1 / 1000  ",
  ] {
    assert_same_fields(&s.parse::<TimeRange>().expect("parses"), &expected);
  }
}

#[test]
fn time_range_parse_rejects_backwards_endpoints() {
  // `TimeRange::new` panics on these and `try_new` returns `None`; parsing
  // must not be a third path that admits them.
  assert_eq!(
    "[3250, 1500) @ 1/1000".parse::<TimeRange>(),
    Err(ParseTimeRangeError(()))
  );
}

#[test]
fn time_range_parse_rejects_malformed_input() {
  for s in [
    "",
    "1500, 3250) @ 1/1000",
    "[1500, 3250 @ 1/1000",
    "[1500 3250) @ 1/1000",
    "[1500, 3250)",
    "[1500, 3250) 1/1000",
    "[1500, 3250) @ 1/0",
    "[1500, 3250, 4000) @ 1/1000",
    "(1500, 3250) @ 1/1000",
  ] {
    assert_eq!(s.parse::<TimeRange>(), Err(ParseTimeRangeError(())), "{s}");
  }
}

#[test]
fn signed_duration_round_trips_through_display() {
  for tb in timebases() {
    for ticks in [0i64, 1, -1, 12_345, -1500, i64::MAX, i64::MIN] {
      let span = SignedDuration::new(ticks, tb);
      // `{}` and `{:#}` are the same rendering for this type, so both invert.
      for rendered in [format!("{span}"), format!("{span:#}")] {
        let parsed: SignedDuration = rendered.parse().expect("Display output parses back");
        assert_eq!(parsed, span);
        assert_same_fields(&parsed, &span);
      }
    }
  }
}

#[test]
fn signed_duration_parse_trims_whitespace_and_reads_a_timebase_name() {
  let expected = SignedDuration::new(-1500, Timebase::MILLIS);
  for s in [
    "-1500 @ 1/1000",
    "-1500@1/1000",
    "  -1500  @  1/1000  ",
    // The timebase half goes through `Timebase`'s impl, so it inherits both
    // the roster arm and its ASCII folding.
    "-1500 @ MILLIS",
    "-1500 @ millis",
  ] {
    assert_same_fields(&s.parse::<SignedDuration>().expect("parses"), &expected);
  }

  // A name on the way in is a rational on the way out, and stays one: the
  // second pass has nothing left to change.
  let once = "-1500 @ MILLIS".parse::<SignedDuration>().expect("parses");
  assert_eq!(format!("{once}"), "-1500 @ 1/1000");
  assert_same_fields(
    &format!("{once}").parse::<SignedDuration>().expect("parses"),
    &once,
  );
}

#[test]
fn signed_duration_and_timestamp_share_a_rendering_and_not_a_parser() {
  // The two exact forms are the same shape, so the string cannot say which
  // type it came from — the type asked for decides, and each rejects with its
  // own error.
  let span = SignedDuration::new(1500, Timebase::MILLIS);
  let instant = Timestamp::new(1500, Timebase::MILLIS);
  assert_eq!(format!("{span}"), format!("{instant:#}"));
  assert_same_fields(
    &format!("{instant:#}")
      .parse::<SignedDuration>()
      .expect("parses"),
    &span,
  );
  assert_eq!(
    "0:00:01.500".parse::<SignedDuration>(),
    Err(ParseSignedDurationError(()))
  );
}

#[test]
fn signed_duration_parse_rejects_malformed_input() {
  for s in [
    "",
    "1500",
    "@1/1000",
    "1500 @",
    "1500 @ 1/1000 @ 2",
    "abc @ 1/1000",
    "1500 @ -1/1000",
    "1.5 @ 1/1000",
    // Beyond `i64`.
    "9223372036854775808 @ 1/1000",
  ] {
    assert_eq!(
      s.parse::<SignedDuration>(),
      Err(ParseSignedDurationError(())),
      "{s}"
    );
  }
}

#[test]
fn duration_round_trips_through_display() {
  for tb in timebases() {
    for ticks in [0u64, 1, 12_345, 1500, u64::MAX] {
      let d = Duration::new(ticks, tb);
      // `{}` and `{:#}` are the same rendering for this type, so both invert.
      for rendered in [format!("{d}"), format!("{d:#}")] {
        let parsed: Duration = rendered.parse().expect("Display output parses back");
        assert_eq!(parsed, d);
        assert_same_fields(&parsed, &d);
      }
    }
  }
}

#[test]
fn duration_parse_trims_whitespace_and_reads_a_timebase_name() {
  let expected = Duration::new(1500, Timebase::MILLIS);
  for s in [
    "1500 @ 1/1000",
    "1500@1/1000",
    "  1500  @  1/1000  ",
    // The timebase half goes through `Timebase`'s impl, so it inherits both
    // the roster arm and its ASCII folding.
    "1500 @ MILLIS",
    "1500 @ millis",
  ] {
    assert_same_fields(&s.parse::<Duration>().expect("parses"), &expected);
  }

  // A name on the way in is a rational on the way out, and stays one: the
  // second pass has nothing left to change.
  let once = "1500 @ MILLIS".parse::<Duration>().expect("parses");
  assert_eq!(format!("{once}"), "1500 @ 1/1000");
  assert_same_fields(
    &format!("{once}").parse::<Duration>().expect("parses"),
    &once,
  );
}

#[test]
fn duration_shares_a_rendering_with_timestamp_and_signed_duration_and_not_a_parser() {
  // Three types, one shape whenever the count is non-negative: the string
  // alone cannot say which was printed, the type asked for decides, and each
  // rejects with its own error.
  let unsigned = Duration::new(1500, Timebase::MILLIS);
  let signed = SignedDuration::new(1500, Timebase::MILLIS);
  let instant = Timestamp::new(1500, Timebase::MILLIS);
  assert_eq!(format!("{unsigned}"), format!("{signed}"));
  assert_eq!(format!("{unsigned}"), format!("{instant:#}"));

  assert_same_fields(
    &format!("{signed}").parse::<Duration>().expect("parses"),
    &unsigned,
  );
  assert_same_fields(
    &format!("{instant:#}").parse::<Duration>().expect("parses"),
    &unsigned,
  );
  assert_eq!(
    "0:00:01.500".parse::<Duration>(),
    Err(ParseDurationError(()))
  );
}

#[test]
fn duration_parse_rejects_malformed_input() {
  for s in [
    "",
    "1500",
    "@1/1000",
    "1500 @",
    "1500 @ 1/1000 @ 2",
    "abc @ 1/1000",
    "1500 @ -1/1000",
    "1.5 @ 1/1000",
    // A `Duration` has no sign to write or to read.
    "-1500 @ 1/1000",
    // Beyond `u64`.
    "18446744073709551616 @ 1/1000",
  ] {
    assert_eq!(s.parse::<Duration>(), Err(ParseDurationError(())), "{s}");
  }
}

#[test]
fn rate_round_trips_through_display() {
  for tb in timebases() {
    let rate = Rate::fps(tb.num(), tb.den());
    for rendered in [format!("{rate}"), format!("{rate:#}")] {
      let parsed: Rate = rendered.parse().expect("Display output parses back");
      assert_eq!(parsed, rate);
      assert_same_fields(&parsed, &rate);
    }
  }
}

#[test]
fn rate_parses_a_well_known_name_on_its_other_arm() {
  for (name, expected) in WELL_KNOWN_RATES {
    assert_same_fields(&name.parse::<Rate>().expect("parses"), expected);
    // The name is an input convenience only: the rendering is the rational.
    assert_eq!(
      format!("{expected}"),
      format!("{}/{}", expected.num(), expected.den())
    );
    // Case folds on this door as it does on `Rate::from_name`.
    for folded in [name.to_ascii_lowercase(), format!("  {name}  ")] {
      assert_same_fields(&folded.parse::<Rate>().expect("parses"), expected);
    }
  }

  assert_eq!("FPS_24".parse::<Rate>(), "24/1".parse::<Rate>());
}

#[test]
fn the_two_rosters_do_not_read_each_other() {
  // A rate and a timebase are reciprocal readings of one rational, so a door
  // that accepted the other's names would answer `1/24` where `24/1` was
  // written. `FILM_24` is `1/24` and `FPS_24` is `24/1` — the pair that makes
  // the mistake concrete.
  for (name, _) in WELL_KNOWN {
    assert_eq!(name.parse::<Rate>(), Err(ParseRateError(())), "{name}");
  }
  for (name, _) in WELL_KNOWN_RATES {
    assert_eq!(
      name.parse::<Timebase>(),
      Err(ParseTimebaseError(())),
      "{name}"
    );
  }

  assert_eq!(Timebase::FILM_24, "FILM_24".parse::<Timebase>().unwrap());
  assert_eq!(
    Rate::FPS_24.to_timebase(),
    "FILM_24".parse::<Timebase>().unwrap()
  );
}

#[test]
fn rate_parse_rejects_what_the_constructor_rejects() {
  for s in ["-1/24", "24/0", "24/-1", "-24/-1"] {
    assert_eq!(s.parse::<Rate>(), Err(ParseRateError(())), "{s}");
  }
}

#[test]
fn rate_parse_rejects_malformed_input() {
  for s in [
    "",
    "/24",
    "24/",
    "24/1/1",
    "a/b",
    "23.976",
    "24.0",
    "-24",
    "24 fps",
    "0x18",
    "24/1 fps",
    "FPS24",
    "fps_23_97",
    "2147483648",
    "2147483648/1",
    "1/2147483648",
  ] {
    assert_eq!(s.parse::<Rate>(), Err(ParseRateError(())), "{s}");
  }
}

#[test]
fn rate_parse_does_not_reduce() {
  // As on the timebase door: `Display` writes the declared form, so the
  // inverse must keep it. `60000/2002` equals `FPS_29_97` under `==`.
  let parsed: Rate = "60000/2002".parse().expect("parses");
  assert_eq!(parsed.num(), 60_000);
  assert_eq!(parsed.den().get(), 2002);
  assert_eq!(parsed, Rate::FPS_29_97);
}

#[test]
fn parse_errors_name_the_grammar_they_wanted() {
  fn message(e: &dyn core::error::Error) -> String {
    format!("{e}")
  }

  assert_eq!(
    message(&ParseTimebaseError(())),
    "expected a timebase `num/den` with num >= 0 and den > 0, or a well-known name"
  );
  assert_eq!(
    message(&ParseTimestampError(())),
    "expected a timestamp `pts @ num/den`"
  );
  assert_eq!(
    message(&ParseSignedDurationError(())),
    "expected a signed duration `ticks @ num/den`"
  );
  assert_eq!(
    message(&ParseDurationError(())),
    "expected a duration `ticks @ num/den`"
  );
  assert_eq!(
    message(&ParseRateError(())),
    "expected a rate `num/den` with num >= 0 and den > 0, a whole number >= 0, or a well-known rate name"
  );
  assert_eq!(
    message(&ParseTimeRangeError(())),
    "expected a time range `[start, end) @ num/den`, with start <= end"
  );
}

#[test]
fn rate_parses_a_whole_number_of_events_per_second() {
  for (s, n) in [("25", 25), (" 48000 ", 48_000), ("+24", 24), ("0", 0)] {
    let rate: Rate = s.parse().expect(s);
    assert_same_fields(&rate, &Rate::hz(n));
    assert_eq!(rate.den().get(), 1);
  }
  assert_eq!("25".parse::<Rate>(), Ok(Rate::FPS_25));
  // `Display` still writes the rational, which parses back to the same rate.
  assert_eq!(format!("{}", "25".parse::<Rate>().unwrap()), "25/1");
  // The timebase door keeps refusing a bare number: it would read as a rate.
  assert_eq!("1000".parse::<Timebase>(), Err(ParseTimebaseError(())));
}

#[test]
fn parse_seconds_reads_decimal_seconds_exactly() {
  let ms = Timebase::MILLIS;
  for (text, timebase, pts) in [
    ("1.5", ms, 1500),
    ("-0.040", ms, -40),
    ("+3", Timebase::SECONDS, 3),
    ("  2.000  ", ms, 2000),
    ("007.50", ms, 7500),
    ("0", ms, 0),
    ("-0", ms, 0),
    ("0.1", Timebase::NANOS, 100_000_000),
    ("1.001", Timebase::NTSC_VIDEO, 30),
    ("-1.001", Timebase::NTSC_VIDEO, -30),
  ] {
    for rounding in [
      Rounding::Exact,
      Rounding::Nearest,
      Rounding::Floor,
      Rounding::Ceil,
    ] {
      assert_eq!(
        Timestamp::parse_seconds(text, timebase, rounding),
        Ok(Timestamp::new(pts, timebase)),
        "{text:?} under {rounding:?}"
      );
    }
  }
  // Trailing zeros change nothing, however many there are.
  let long_one = format!("1.{}", "0".repeat(60));
  assert_eq!(
    Timestamp::parse_seconds(&long_one, ms, Rounding::Exact),
    Ok(Timestamp::new(1000, ms))
  );
}

#[test]
fn parse_seconds_rounds_the_way_it_is_told() {
  let ms = Timebase::MILLIS;
  // Half a millisecond either side of zero: the tie goes away from zero
  // under `Nearest`, and floor and ceiling keep their direction.
  for (text, floor, ceil, nearest) in [("0.0005", 0, 1, 1), ("-0.0005", -1, 0, -1)] {
    let read = |rounding| Timestamp::parse_seconds(text, ms, rounding).map(|t| t.pts());
    assert_eq!(read(Rounding::Floor), Ok(floor), "{text}");
    assert_eq!(read(Rounding::Ceil), Ok(ceil), "{text}");
    assert_eq!(read(Rounding::Nearest), Ok(nearest), "{text}");
    assert_eq!(
      read(Rounding::Exact),
      Err(ParseSecondsError::BetweenTicks),
      "{text}"
    );
  }
  // One second at 29.97 fps is 29.97 frames.
  let ntsc = Timebase::NTSC_VIDEO;
  let read = |rounding| Timestamp::parse_seconds("1", ntsc, rounding).map(|t| t.pts());
  assert_eq!(read(Rounding::Floor), Ok(29));
  assert_eq!(read(Rounding::Ceil), Ok(30));
  assert_eq!(read(Rounding::Nearest), Ok(30));
  assert_eq!(read(Rounding::Exact), Err(ParseSecondsError::BetweenTicks));
}

#[test]
fn parse_seconds_refuses_by_name() {
  let ms = Timebase::MILLIS;
  for text in [
    "",
    " ",
    "+",
    "-",
    ".",
    ".5",
    "5.",
    "1e3",
    "1E3",
    "1_000",
    "inf",
    "NaN",
    "1.2.3",
    "--1",
    "+-1",
    "1 .5",
    "1. 5",
    "0:00:01.000",
    "1s",
    "0x10",
    "١",
  ] {
    assert_eq!(
      Timestamp::parse_seconds(text, ms, Rounding::Nearest),
      Err(ParseSecondsError::NotDecimal),
      "{text:?}"
    );
  }

  let zero = Timebase::new(0, nz(1));
  assert_eq!(
    Timestamp::parse_seconds("1", zero, Rounding::Nearest),
    Err(ParseSecondsError::DegenerateTimebase)
  );
  assert_eq!(
    Timestamp::parse_seconds("0", zero, Rounding::Exact),
    Err(ParseSecondsError::DegenerateTimebase)
  );
  // The text is judged first.
  assert_eq!(
    Timestamp::parse_seconds("x", zero, Rounding::Nearest),
    Err(ParseSecondsError::NotDecimal)
  );

  // Past i64 in the timebase, whether or not the count is whole, and past
  // what an exact i128 reading holds.
  assert_eq!(
    Timestamp::parse_seconds("9223372036854775808", Timebase::SECONDS, Rounding::Exact),
    Err(ParseSecondsError::OutOfRange)
  );
  assert_eq!(
    Timestamp::parse_seconds("9223372036854775807.5", Timebase::SECONDS, Rounding::Ceil),
    Err(ParseSecondsError::OutOfRange)
  );
  assert_eq!(
    Timestamp::parse_seconds("9223372036854775807", Timebase::SECONDS, Rounding::Exact),
    Ok(Timestamp::new(i64::MAX, Timebase::SECONDS))
  );
  let too_many_digits = format!("1{}", "0".repeat(39));
  assert_eq!(
    Timestamp::parse_seconds(&too_many_digits, ms, Rounding::Floor),
    Err(ParseSecondsError::OutOfRange)
  );
  let too_fine = format!("0.{}1", "0".repeat(38));
  assert_eq!(
    Timestamp::parse_seconds(&too_fine, ms, Rounding::Floor),
    Err(ParseSecondsError::OutOfRange)
  );
}

#[test]
fn parse_seconds_errors_say_what_failed() {
  fn message(e: &dyn core::error::Error) -> String {
    format!("{e}")
  }
  assert_eq!(
    message(&ParseSecondsError::NotDecimal),
    "expected decimal seconds: an optional sign, digits, and optionally a point and digits"
  );
  assert_eq!(
    message(&ParseSecondsError::BetweenTicks),
    "the seconds fall between two ticks of the timebase, and an exact reading was asked for"
  );
  assert_eq!(
    message(&ParseSecondsError::OutOfRange),
    "the seconds are out of range for a count of the timebase's ticks"
  );
  assert_eq!(
    message(&ParseSecondsError::DegenerateTimebase),
    "the timebase's numerator is zero, so no count of its ticks measures seconds"
  );
}
