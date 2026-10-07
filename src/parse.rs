//! [`FromStr`] for the six time types, and the errors they reject with.
//!
//! Each impl is the inverse of the type's **exact** rendering — the one
//! `{:#}` writes, which is `{}` as well for [`Timebase`], [`Rate`],
//! [`SignedDuration`] and [`Duration`], whose renderings have nothing to
//! expand into. That is the only rendering an inverse can exist for:
//! [`Timestamp`]'s and [`TimeRange`]'s default `{}` form is a clock truncated
//! to milliseconds that never names a timebase, so two different instants can
//! share one rendering and no parser can tell which was meant. Accepting it
//! would mint a value that does not compare equal to the one printed. See
//! each impl for its grammar.
//!
//! Each type rejects with **its own** error, named for the vocabulary it
//! wanted: a caller matching on a failed `Rate` parse should not have to read
//! a message about timebases, and the two rosters are disjoint on purpose.
//!
//! Beside the six inverses, [`Timestamp::parse_seconds`] reads decimal
//! seconds a person wrote. It is not a `FromStr`, because it inverts no
//! rendering and the text alone does not say enough: seconds name an instant
//! but not the timebase to count it in, nor which way to round when it falls
//! between two ticks, so the caller names both.

use core::{fmt, num::NonZeroI32, str::FromStr};

use crate::{
  Duration, ExactSeconds, Rate, Rounding, SignedDuration, TimeRange, Timebase, Timestamp,
};

/// The `num/den` half of the two rational grammars, scanned once so
/// [`Timebase`] and [`Rate`] cannot drift apart in what they accept.
///
/// Only the shape is decided here. The caller applies its own constructor as
/// the validator, because the invariants land in different types and are
/// reported under different errors.
fn rational(s: &str) -> Option<(i32, NonZeroI32)> {
  let (num, den) = s.split_once('/')?;
  let num = num.trim().parse::<i32>().ok()?;
  let den = den.trim().parse::<i32>().ok()?;
  Some((num, NonZeroI32::new(den)?))
}

/// Returned when a string is not a [`Timebase`] rendering.
///
/// Carries no detail: the grammar is two integers and a slash, or a name from
/// a fixed roster, so the input is its own diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseTimebaseError(());

impl fmt::Display for ParseTimebaseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("expected a timebase `num/den` with num >= 0 and den > 0, or a well-known name")
  }
}

impl core::error::Error for ParseTimebaseError {}

/// Returned when a string is not a [`Timestamp`] rendering.
///
/// Also returned for the readable `H:MM:SS.mmm` clock form, which is lossy
/// and therefore not parsed — see [`Timestamp`]'s `Display` impl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseTimestampError(());

impl fmt::Display for ParseTimestampError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("expected a timestamp `pts @ num/den`")
  }
}

impl core::error::Error for ParseTimestampError {}

/// Returned when a string is not a [`SignedDuration`] rendering.
///
/// Distinct from [`ParseTimestampError`] although the two grammars are the
/// same shape: a count and an instant are different vocabularies, and the
/// message says which one was expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseSignedDurationError(());

impl fmt::Display for ParseSignedDurationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("expected a signed duration `ticks @ num/den`")
  }
}

impl core::error::Error for ParseSignedDurationError {}

/// Returned when a string is not a [`Duration`] rendering.
///
/// Distinct from [`ParseSignedDurationError`] although the two grammars are
/// the same shape: an unsigned span and a signed one are different
/// vocabularies, and the message says which one was expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseDurationError(());

impl fmt::Display for ParseDurationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("expected a duration `ticks @ num/den`")
  }
}

impl core::error::Error for ParseDurationError {}

/// Returned when a string is not a [`Rate`] rendering.
///
/// Carries no detail, as [`ParseTimebaseError`] does not: the grammar is two
/// integers and a slash, one whole number, or a name from a fixed roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseRateError(());

impl fmt::Display for ParseRateError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(
      "expected a rate `num/den` with num >= 0 and den > 0, a whole number >= 0, or a well-known rate name",
    )
  }
}

impl core::error::Error for ParseRateError {}

/// Returned when a string is not a [`TimeRange`] rendering.
///
/// Also returned when the endpoints parse but run backwards, which
/// [`TimeRange::try_new`] rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseTimeRangeError(());

impl fmt::Display for ParseTimeRangeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("expected a time range `[start, end) @ num/den`, with start <= end")
  }
}

impl core::error::Error for ParseTimeRangeError {}

/// Why [`Timestamp::parse_seconds`] refused its text.
///
/// Unlike the other parse errors here, this one says *how* the text failed.
/// The grammar is still one number, but a number that reads can be refused
/// for where it lands, and a caller who asked for [`Rounding::Exact`] needs
/// to tell "not a number" from "between two ticks".
///
/// Marked `#[non_exhaustive]`: a reason added later must not break a `match`
/// written against these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ParseSecondsError {
  /// The text is not decimal seconds: an optional sign, one or more digits,
  /// and optionally a point followed by one or more digits — `12`, `-0.5`,
  /// `+3.040`.
  NotDecimal,
  /// The seconds fall between two ticks of the timebase — both within what
  /// an `i64` counts — and [`Rounding::Exact`] was asked for, which refuses
  /// to pick one.
  BetweenTicks,
  /// The seconds are past what an `i64` count of the timebase's ticks
  /// reaches, or have more significant digits than an exact `i128` reading
  /// holds (38).
  OutOfRange,
  /// The timebase is degenerate (`num() == 0`): it names one instant and can
  /// count no other.
  DegenerateTimebase,
}

impl fmt::Display for ParseSecondsError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::NotDecimal => {
        "expected decimal seconds: an optional sign, digits, and optionally a point and digits"
      }
      Self::BetweenTicks => {
        "the seconds fall between two ticks of the timebase, and an exact reading was asked for"
      }
      Self::OutOfRange => "the seconds are out of range for a count of the timebase's ticks",
      Self::DegenerateTimebase => {
        "the timebase's numerator is zero, so no count of its ticks measures seconds"
      }
    })
  }
}

impl core::error::Error for ParseSecondsError {}
/// Parses either a [well-known name](Timebase#the-well-known-roster) —
/// `MILLIS`, `MPEG_90K` — or `num/den`, the form [`Timebase`]'s `Display`
/// writes in both `{}` and `{:#}`.
///
/// The roster is tried first, via [`Timebase::from_name`], so the name arm
/// folds ASCII case as that door does — `millis` parses — and nothing else:
/// no alias, no separator guessing. Nothing in the roster contains a slash, so
/// the two arms cannot collide. It is an *input* convenience for
/// hand-written configuration
/// and command lines: `Display` still writes `num/den` for every value, so the
/// `Display` → `FromStr` round trip is unchanged and lossless. The reverse is
/// deliberately not injective — `"MILLIS"` and `"1/1000"` parse to the same
/// timebase, and [`Timebase::well_known_name`] is where the name goes to be
/// recovered.
///
/// Surrounding and interior whitespace is trimmed, so `1 / 1000` parses; on
/// the `num/den` arm the slash is required. The value is **not** reduced:
/// `2/4` parses to a numerator of 2 over a denominator of 4, which is what was
/// written, and what `Display` will write back.
///
/// [`Timestamp`], [`TimeRange`], [`SignedDuration`] and [`Duration`] parse
/// their timebase half through this impl, so `12345 @ MPEG_90K` parses too.
/// [`Rate`]'s roster is **not** read here, nor this one there — see that impl
/// for why.
///
/// # Errors
///
/// Returns [`ParseTimebaseError`] if the input is neither a roster name nor a
/// `num/den` pair: the slash is missing, either half is not an `i32`, or the
/// pair is one [`Timebase::try_new`] refuses — a negative numerator, or a
/// denominator that is zero or negative.
impl FromStr for Timebase {
  type Err = ParseTimebaseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let s = s.trim();
    if let Some(known) = Self::from_name(s) {
      return Ok(known);
    }
    rational(s)
      .and_then(|(num, den)| Self::try_new(num, den))
      .ok_or(ParseTimebaseError(()))
  }
}

/// Parses a [well-known rate name](Rate#the-well-known-roster) — `FPS_29_97`,
/// `FPS_24` — or `num/den`, the form [`Rate`]'s `Display` writes in both `{}`
/// and `{:#}`, or a whole number of events per second: `25` is `25/1`.
///
/// The two arms and their order are [`Timebase`]'s, over the *rate* roster:
/// the name arm is tried first, through [`Rate::from_name`], so it folds
/// ASCII case — `fps_29_97` parses — and nothing else. No rate name contains a
/// slash, so the arms cannot collide.
///
/// The rosters, though, are **disjoint on purpose**: `"MILLIS"` is not a rate
/// and `"FPS_24"` is not a timebase, and each door refuses the other's names.
/// A rate and a timebase are reciprocal readings of one rational, so a door
/// that read both would silently answer `1/24` where `24/1` was written.
/// [`Rate::to_timebase`] is the conversion, and it is asked for.
///
/// Whitespace is trimmed as it is on the timebase door, the value is not
/// reduced, and the name arm is an input convenience only: `Display` writes
/// `num/den` for every value, so the `Display` → `FromStr` round trip is
/// lossless and `"FPS_24"` and `"24/1"` land on the same rate.
///
/// The whole-number arm is how rates are usually written — `25`, `48000` —
/// and reads through [`Rate::try_hz`], so `0` is the degenerate rate as `0/1`
/// is. A **decimal** is refused: `23.976` is not `24000/1001` but a different
/// rate, a millionth slower, that drifts from it by 3.6 ms an hour — and
/// guessing which rational was meant is the silent approximation this crate
/// exists to refuse. The
/// [`Timebase`] door takes no bare number at all: `1000` written where a
/// timebase is wanted reads as a rate, and answering `1000/1` — or `1/1000` —
/// would make that reciprocal mistake for the caller.
///
/// # Errors
///
/// Returns [`ParseRateError`] if the input is not a rate name, a `num/den`
/// pair or a whole number: a half or the number is not an `i32`, or the value
/// is one [`Rate::try_fps`] refuses — a negative numerator, or a denominator
/// that is zero or negative.
impl FromStr for Rate {
  type Err = ParseRateError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let s = s.trim();
    if let Some(known) = Self::from_name(s) {
      return Ok(known);
    }
    if s.contains('/') {
      return rational(s)
        .and_then(|(num, den)| Self::try_fps(num, den))
        .ok_or(ParseRateError(()));
    }
    s.parse::<i32>()
      .ok()
      .and_then(Self::try_hz)
      .ok_or(ParseRateError(()))
  }
}

/// Parses `pts @ num/den` — the form [`Timestamp`]'s `Display` writes under
/// `{:#}`.
///
/// Whitespace around each part is trimmed, so `12345@1/90000` parses as well
/// as `12345 @ 1/90000`. The default `{}` clock is **not** accepted: it is
/// truncated to milliseconds and names no timebase, so it cannot name back
/// the instant it was printed from.
///
/// # Errors
///
/// Returns [`ParseTimestampError`] if the `@` is missing, if the PTS is not
/// an `i64`, or if the timebase half is not one [`Timebase`] accepts.
impl FromStr for Timestamp {
  type Err = ParseTimestampError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let err = ParseTimestampError(());
    let (pts, timebase) = s.split_once('@').ok_or(err)?;
    let pts = pts.trim().parse::<i64>().map_err(|_| err)?;
    let timebase = timebase.trim().parse::<Timebase>().map_err(|_| err)?;
    Ok(Self::new(pts, timebase))
  }
}

/// Parses `ticks @ num/den` — the form [`SignedDuration`]'s `Display` writes,
/// under both `{}` and `{:#}`.
///
/// [`Timestamp`]'s grammar over a count: whitespace around each part is
/// trimmed, the timebase half goes through [`Timebase`]'s own impl, so
/// `-1500 @ MILLIS` parses, and the leading `-` is the count's, a timebase
/// having no sign to write.
///
/// The rendering is the same shape as [`Timestamp`]'s `{:#}`, so a string
/// alone does not say which type was printed; the type asked for decides, and
/// `"1500 @ 1/1000".parse::<SignedDuration>()` is a span however the string
/// was produced.
///
/// # Errors
///
/// Returns [`ParseSignedDurationError`] if the `@` is missing, if the count is
/// not an `i64`, or if the timebase half is not one [`Timebase`] accepts.
impl FromStr for SignedDuration {
  type Err = ParseSignedDurationError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let err = ParseSignedDurationError(());
    let (ticks, timebase) = s.split_once('@').ok_or(err)?;
    let ticks = ticks.trim().parse::<i64>().map_err(|_| err)?;
    let timebase = timebase.trim().parse::<Timebase>().map_err(|_| err)?;
    Ok(Self::new(ticks, timebase))
  }
}

/// Parses `ticks @ num/den` — the form [`Duration`]'s `Display` writes, under
/// both `{}` and `{:#}`.
///
/// [`SignedDuration`]'s grammar over an unsigned count: whitespace around
/// each part is trimmed, the timebase half goes through [`Timebase`]'s own
/// impl, so `1500 @ MILLIS` parses. There is no leading `-` to trim here — a
/// [`Duration`] never has one to write.
///
/// # Errors
///
/// Returns [`ParseDurationError`] if the `@` is missing, if the count is not
/// a `u64`, or if the timebase half is not one [`Timebase`] accepts.
impl FromStr for Duration {
  type Err = ParseDurationError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let err = ParseDurationError(());
    let (ticks, timebase) = s.split_once('@').ok_or(err)?;
    let ticks = ticks.trim().parse::<u64>().map_err(|_| err)?;
    let timebase = timebase.trim().parse::<Timebase>().map_err(|_| err)?;
    Ok(Self::new(ticks, timebase))
  }
}

/// Parses `[start, end) @ num/den` — the form [`TimeRange`]'s `Display`
/// writes under `{:#}`.
///
/// The half-open brackets are required, in that asymmetry, because they are
/// what the rendering means. Whitespace around each part is trimmed. The
/// default `{}` form, a pair of clocks, is **not** accepted, for the reason
/// [`Timestamp`]'s is not.
///
/// # Errors
///
/// Returns [`ParseTimeRangeError`] if the bracket, comma, or `@` is missing,
/// if an endpoint is not an `i64`, if the timebase half is not one
/// [`Timebase`] accepts, or if the endpoints run backwards.
impl FromStr for TimeRange {
  type Err = ParseTimeRangeError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let err = ParseTimeRangeError(());
    let body = s.trim().strip_prefix('[').ok_or(err)?;
    let (endpoints, tail) = body.split_once(')').ok_or(err)?;
    let (start, end) = endpoints.split_once(',').ok_or(err)?;
    let start = start.trim().parse::<i64>().map_err(|_| err)?;
    let end = end.trim().parse::<i64>().map_err(|_| err)?;
    let timebase = tail
      .trim()
      .strip_prefix('@')
      .ok_or(err)?
      .trim()
      .parse::<Timebase>()
      .map_err(|_| err)?;
    Self::try_new(start, end, timebase).ok_or(err)
  }
}

impl Timestamp {
  /// Reads decimal seconds — `12.5`, `-0.040`, `3` — as an instant counted in
  /// `timebase`, from PTS zero.
  ///
  /// The digits are read exactly, as integers, never through a float: `0.1`
  /// is one tenth of a second, not the double nearest it. Seconds that land on
  /// a tick of `timebase` are that tick under every rounding; seconds between
  /// two ticks go where `rounding` says — or, under [`Rounding::Exact`], are
  /// refused by name, with [`ParseSecondsError::BetweenTicks`].
  ///
  /// ```
  /// use mediatime::{ParseSecondsError, Rounding, Timebase, Timestamp};
  ///
  /// let ntsc = Timebase::NTSC_VIDEO; // 1001/30000 s per frame
  /// // 1.001 s is 30 frames exactly; 1 s is 29.97 frames.
  /// let at = Timestamp::parse_seconds("1.001", ntsc, Rounding::Exact);
  /// assert_eq!(at, Ok(Timestamp::new(30, ntsc)));
  /// let start = Timestamp::parse_seconds("1", ntsc, Rounding::Ceil);
  /// assert_eq!(start, Ok(Timestamp::new(30, ntsc)));
  /// assert_eq!(
  ///   Timestamp::parse_seconds("1", ntsc, Rounding::Exact),
  ///   Err(ParseSecondsError::BetweenTicks)
  /// );
  /// ```
  ///
  /// The grammar is one decimal number: an optional `+` or `-`, one or more
  /// ASCII digits, and optionally a `.` followed by one or more digits.
  /// Surrounding whitespace is trimmed. There is no exponent, no digit
  /// separator, no bare `.5` or `5.`, and no `inf` or `NaN`.
  ///
  /// Nor is there a clock. The `H:MM:SS.mmm` form [`Timestamp`]'s `Display`
  /// writes is truncated to the millisecond and names no timebase, so it has
  /// no inverse — here or on the `FromStr` door — and a clock a person typed
  /// is a second grammar, sexagesimal, which a caller can turn into seconds
  /// before it gets here.
  ///
  /// # Errors
  ///
  /// - [`ParseSecondsError::NotDecimal`] for text outside the grammar;
  /// - [`ParseSecondsError::DegenerateTimebase`] for a `timebase` whose
  ///   numerator is zero, which can count no instant;
  /// - [`ParseSecondsError::BetweenTicks`] under [`Rounding::Exact`], for
  ///   seconds between two ticks that an `i64` both counts;
  /// - [`ParseSecondsError::OutOfRange`] for a count past `i64`, or for more
  ///   significant digits than an exact `i128` reading holds.
  pub fn parse_seconds(
    text: &str,
    timebase: Timebase,
    rounding: Rounding,
  ) -> Result<Self, ParseSecondsError> {
    let seconds = decimal_seconds(text)?;
    if timebase.num() == 0 {
      return Err(ParseSecondsError::DegenerateTimebase);
    }
    match seconds.checked_to_timestamp(timebase, rounding) {
      Some(at) => Ok(at),
      // Between two ticks only if both are ticks an `i64` counts: past the
      // last one, the instant is out of range whichever way it would round.
      None
        if matches!(rounding, Rounding::Exact)
          && seconds
            .checked_to_timestamp(timebase, Rounding::Floor)
            .is_some()
          && seconds
            .checked_to_timestamp(timebase, Rounding::Ceil)
            .is_some() =>
      {
        Err(ParseSecondsError::BetweenTicks)
      }
      None => Err(ParseSecondsError::OutOfRange),
    }
  }
}

/// `text` as exact seconds, by the grammar [`Timestamp::parse_seconds`]
/// documents: the digits accumulate into one `i128` numerator over a power of
/// ten, after the fraction's trailing zeros are dropped — they change nothing,
/// and keeping them would spend the `i128`'s room on nothing.
fn decimal_seconds(text: &str) -> Result<ExactSeconds, ParseSecondsError> {
  fn digits(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
  }

  let text = text.trim();
  let (negative, unsigned) = match text.as_bytes().first() {
    Some(b'-') => (true, &text[1..]),
    Some(b'+') => (false, &text[1..]),
    _ => (false, text),
  };
  let (whole, fraction) = match unsigned.split_once('.') {
    Some((whole, fraction)) if digits(whole) && digits(fraction) => {
      (whole, fraction.trim_end_matches('0'))
    }
    None if digits(unsigned) => (unsigned, ""),
    _ => return Err(ParseSecondsError::NotDecimal),
  };

  let mut num: i128 = 0;
  for b in whole.bytes().chain(fraction.bytes()) {
    num = num
      .checked_mul(10)
      .and_then(|n| n.checked_add((b - b'0') as i128))
      .ok_or(ParseSecondsError::OutOfRange)?;
  }
  let den = u32::try_from(fraction.len())
    .ok()
    .and_then(|places| 10_i128.checked_pow(places))
    .ok_or(ParseSecondsError::OutOfRange)?;
  Ok(ExactSeconds::reduced(
    if negative { -num } else { num },
    den,
  ))
}
#[cfg(test)]
mod tests;
