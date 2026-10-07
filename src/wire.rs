//! The `mediatime.v1` protobuf package as the wire carries it, for buffa's
//! `extern_path` — the mapping that keeps protobuf's semantics.
//!
//! `extern_path(".mediatime.v1", "::mediatime::wire")` maps the package
//! here. Each type is its message exactly as protobuf reads it: every field a
//! plain scalar or message, starting from protobuf's zero state and merged
//! occurrence by occurrence, with nothing judged, repaired or refused while
//! it decodes. A proto3 encoder writes `0/3` as the denominator alone, and it
//! reads back here as `0/3`; a peer's zero denominator stays zero; a range
//! field split over occurrences — `{start: 100}`, then `{end: 200}` — merges
//! into `[100, 200)`, though its first part alone is inverted.
//!
//! The domain types are reached by checked conversions once the enclosing
//! message is decoded, and [`ConversionError`] names what a value fails:
//!
//! ```
//! use buffa::Message;
//! use mediatime::{Timebase, wire};
//!
//! let range = mediatime::TimeRange::new(100, 200, Timebase::MILLIS);
//! let bytes = wire::TimeRange::from(range).encode_to_vec();
//! let decoded = wire::TimeRange::decode_from_slice(&bytes).unwrap();
//! assert_eq!(mediatime::TimeRange::try_from(decoded), Ok(range));
//!
//! // A zero numerator elided by a proto3 encoder reads back as zero.
//! let timebase = wire::Timebase::decode_from_slice(&[0x10, 0x03]).unwrap();
//! assert_eq!((timebase.num, timebase.den), (0, 3));
//! ```
//!
//! Mapping the package onto the crate root instead (`"::mediatime"`) decodes
//! straight into the domain types, through their own `Message` impls. Those
//! cannot see protobuf's zero state or the end of an enclosing message: a
//! `Timebase` starts from `1/1` and repairs a malformed field as it lands, and
//! a `TimeRange` judges each occurrence on its own.

use core::{fmt, num::NonZeroI32};

use ::buffa::{
  DecodeContext, DecodeError, DefaultInstance, EncodeSink, Message, SizeCache,
  bytes::Buf,
  encoding::{Tag, WireType, encode_varint, skip_field_depth, varint_len},
  types::{
    decode_int32, decode_int64, encode_int32, encode_int64, int32_encoded_len, int64_encoded_len,
  },
};

/// A `mediatime.v1.Timebase` message as read from the wire: two `int32`
/// fields, zero when absent, kept as written.
///
/// [`crate::Timebase`] holds only a non-negative numerator over a positive
/// denominator; `crate::Timebase::try_from` is the checked conversion, and
/// a zero numerator passes it (see [why it is
/// legal](crate::Timebase#why-a-zero-numerator-is-legal)). The encoder writes
/// both fields always, as the domain encoder does, so a reader that starts
/// from `1/1` still reads a zero numerator back.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timebase {
  /// Field 1, `int32 num`.
  pub num: i32,
  /// Field 2, `int32 den`.
  pub den: i32,
}

/// A `mediatime.v1.Timestamp` message as read from the wire: a count and a
/// [`Timebase`] message, zero when absent, kept as written.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp {
  /// Field 1, `int64 pts`.
  pub pts: i64,
  /// Field 2, the `Timebase` message, always encoded.
  pub timebase: Timebase,
}

/// A `mediatime.v1.TimeRange` message as merged from the wire: two endpoints
/// and a [`Timebase`] message, with no order between the endpoints.
///
/// It decodes with protobuf's merge semantics unchanged — a field present in
/// a later occurrence replaces the earlier value, an absent one keeps it —
/// and it may therefore hold `end < start`, which [`crate::TimeRange`] never
/// does. `crate::TimeRange::try_from` is the checked conversion, and
/// `From<crate::TimeRange>` the total one back. The encoding is the domain
/// type's, byte for byte.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeRange {
  /// Field 1, `int64 start`.
  pub start: i64,
  /// Field 2, `int64 end`.
  pub end: i64,
  /// Field 3, the `Timebase` message, always encoded.
  pub timebase: Timebase,
}

impl From<crate::Timebase> for Timebase {
  #[cfg_attr(not(tarpaulin), inline(always))]
  fn from(timebase: crate::Timebase) -> Self {
    Self {
      num: timebase.num(),
      den: timebase.den().get(),
    }
  }
}

impl From<crate::Timestamp> for Timestamp {
  #[cfg_attr(not(tarpaulin), inline(always))]
  fn from(timestamp: crate::Timestamp) -> Self {
    Self {
      pts: timestamp.pts(),
      timebase: timestamp.timebase().into(),
    }
  }
}

impl From<crate::TimeRange> for TimeRange {
  #[cfg_attr(not(tarpaulin), inline(always))]
  fn from(range: crate::TimeRange) -> Self {
    Self {
      start: range.start_pts(),
      end: range.end_pts(),
      timebase: range.timebase().into(),
    }
  }
}

/// What a wire value fails when it is converted into its domain type.
///
/// Marked `#[non_exhaustive]`: a reason added later must not break a `match`
/// written against these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConversionError {
  /// The timebase's numerator is negative.
  NegativeNumerator,
  /// The timebase's denominator is zero — or absent, which protobuf reads as
  /// zero.
  ZeroDenominator,
  /// The timebase's denominator is negative.
  NegativeDenominator,
  /// The range's `end` precedes its `start`.
  InvertedRange,
}

impl fmt::Display for ConversionError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::NegativeNumerator => "timebase numerator is negative",
      Self::ZeroDenominator => "timebase denominator is zero",
      Self::NegativeDenominator => "timebase denominator is negative",
      Self::InvertedRange => "time range end precedes its start",
    })
  }
}

impl core::error::Error for ConversionError {}

/// The checked conversion: the wire timebase as a [`crate::Timebase`], or the
/// half that fails it. A zero numerator passes.
impl TryFrom<Timebase> for crate::Timebase {
  type Error = ConversionError;

  fn try_from(timebase: Timebase) -> Result<Self, ConversionError> {
    if timebase.num < 0 {
      return Err(ConversionError::NegativeNumerator);
    }
    match NonZeroI32::new(timebase.den) {
      None => Err(ConversionError::ZeroDenominator),
      Some(den) if den.get() < 0 => Err(ConversionError::NegativeDenominator),
      Some(den) => Ok(Self::new(timebase.num, den)),
    }
  }
}

/// The checked conversion: the wire timestamp as a [`crate::Timestamp`], or
/// what its timebase fails. Every count is a legal PTS.
impl TryFrom<Timestamp> for crate::Timestamp {
  type Error = ConversionError;

  fn try_from(timestamp: Timestamp) -> Result<Self, ConversionError> {
    Ok(Self::new(timestamp.pts, timestamp.timebase.try_into()?))
  }
}

/// The checked conversion: the wire range as a [`crate::TimeRange`] — its
/// timebase judged first, then the order of its endpoints.
impl TryFrom<TimeRange> for crate::TimeRange {
  type Error = ConversionError;

  fn try_from(range: TimeRange) -> Result<Self, ConversionError> {
    let timebase = range.timebase.try_into()?;
    Self::try_new(range.start, range.end, timebase).ok_or(ConversionError::InvertedRange)
  }
}

const VARINT: u8 = WireType::Varint as u8;
const LEN: u8 = WireType::LengthDelimited as u8;

impl DefaultInstance for Timebase {
  fn default_instance() -> &'static Self {
    static VALUE: buffa::__private::OnceBox<Timebase> = buffa::__private::OnceBox::new();
    VALUE.get_or_init(|| buffa::alloc::boxed::Box::new(Timebase::default()))
  }
}

impl Message for Timebase {
  // Both fields are written always — proto3 allows it — so that a reader
  // starting from `1/1`, as the domain decoder does, reads a zero numerator
  // back rather than its own default. Both tags are single-byte.
  fn compute_size(&self, _cache: &mut SizeCache) -> u32 {
    2 + int32_encoded_len(self.num) as u32 + int32_encoded_len(self.den) as u32
  }

  fn write_to(&self, _cache: &mut SizeCache, buf: &mut impl EncodeSink) {
    Tag::new(1, WireType::Varint).encode(buf);
    encode_int32(self.num, buf);
    Tag::new(2, WireType::Varint).encode(buf);
    encode_int32(self.den, buf);
  }

  fn merge_field(
    &mut self,
    tag: Tag,
    buf: &mut impl Buf,
    ctx: DecodeContext<'_>,
  ) -> Result<(), DecodeError> {
    match tag.field_number() {
      1 => {
        if tag.wire_type() != WireType::Varint {
          return Err(DecodeError::WireTypeMismatch {
            field_number: 1,
            expected: VARINT,
            actual: tag.wire_type() as u8,
          });
        }
        self.num = decode_int32(buf)?;
      }
      2 => {
        if tag.wire_type() != WireType::Varint {
          return Err(DecodeError::WireTypeMismatch {
            field_number: 2,
            expected: VARINT,
            actual: tag.wire_type() as u8,
          });
        }
        self.den = decode_int32(buf)?;
      }
      _ => skip_field_depth(tag, buf, ctx.depth())?,
    }
    Ok(())
  }

  fn clear(&mut self) {
    *self = Timebase::default();
  }
}

impl DefaultInstance for Timestamp {
  fn default_instance() -> &'static Self {
    static VALUE: buffa::__private::OnceBox<Timestamp> = buffa::__private::OnceBox::new();
    VALUE.get_or_init(|| buffa::alloc::boxed::Box::new(Timestamp::default()))
  }
}

impl Message for Timestamp {
  fn compute_size(&self, cache: &mut SizeCache) -> u32 {
    let mut size = 0u32;
    // proto3 zero-elision: every reader seeds `pts` at 0.
    if self.pts != 0 {
      size += 1 + int64_encoded_len(self.pts) as u32;
    }
    let slot = cache.reserve();
    let inner = self.timebase.compute_size(cache);
    cache.set(slot, inner);
    size += 1 + varint_len(inner as u64) as u32 + inner;
    size
  }

  fn write_to(&self, cache: &mut SizeCache, buf: &mut impl EncodeSink) {
    if self.pts != 0 {
      Tag::new(1, WireType::Varint).encode(buf);
      encode_int64(self.pts, buf);
    }
    Tag::new(2, WireType::LengthDelimited).encode(buf);
    encode_varint(cache.consume_next() as u64, buf);
    self.timebase.write_to(cache, buf);
  }

  fn merge_field(
    &mut self,
    tag: Tag,
    buf: &mut impl Buf,
    ctx: DecodeContext<'_>,
  ) -> Result<(), DecodeError> {
    match tag.field_number() {
      1 => {
        if tag.wire_type() != WireType::Varint {
          return Err(DecodeError::WireTypeMismatch {
            field_number: 1,
            expected: VARINT,
            actual: tag.wire_type() as u8,
          });
        }
        self.pts = decode_int64(buf)?;
      }
      2 => {
        if tag.wire_type() != WireType::LengthDelimited {
          return Err(DecodeError::WireTypeMismatch {
            field_number: 2,
            expected: LEN,
            actual: tag.wire_type() as u8,
          });
        }
        buffa::Message::merge_length_delimited(&mut self.timebase, buf, ctx)?;
      }
      _ => skip_field_depth(tag, buf, ctx.depth())?,
    }
    Ok(())
  }

  fn clear(&mut self) {
    *self = Timestamp::default();
  }
}

impl DefaultInstance for TimeRange {
  fn default_instance() -> &'static Self {
    static VALUE: buffa::__private::OnceBox<TimeRange> = buffa::__private::OnceBox::new();
    VALUE.get_or_init(|| buffa::alloc::boxed::Box::new(TimeRange::default()))
  }
}

impl Message for TimeRange {
  fn compute_size(&self, cache: &mut SizeCache) -> u32 {
    let mut size = 0u32;
    // proto3 zero-elision: sound here — the decoder seeds start/end at 0.
    if self.start != 0 {
      size += 1 + int64_encoded_len(self.start) as u32;
    }
    if self.end != 0 {
      size += 1 + int64_encoded_len(self.end) as u32;
    }
    // timebase (field 3) — always encoded for unconditional round-trip.
    let slot = cache.reserve();
    let inner = self.timebase.compute_size(cache);
    cache.set(slot, inner);
    size += 1 + varint_len(inner as u64) as u32 + inner;
    size
  }

  fn write_to(&self, cache: &mut SizeCache, buf: &mut impl EncodeSink) {
    if self.start != 0 {
      Tag::new(1, WireType::Varint).encode(buf);
      encode_int64(self.start, buf);
    }
    if self.end != 0 {
      Tag::new(2, WireType::Varint).encode(buf);
      encode_int64(self.end, buf);
    }
    Tag::new(3, WireType::LengthDelimited).encode(buf);
    encode_varint(cache.consume_next() as u64, buf);
    self.timebase.write_to(cache, buf);
  }

  fn merge_field(
    &mut self,
    tag: Tag,
    buf: &mut impl Buf,
    ctx: DecodeContext<'_>,
  ) -> Result<(), DecodeError> {
    match tag.field_number() {
      1 => {
        if tag.wire_type() != WireType::Varint {
          return Err(DecodeError::WireTypeMismatch {
            field_number: 1,
            expected: VARINT,
            actual: tag.wire_type() as u8,
          });
        }
        self.start = decode_int64(buf)?;
      }
      2 => {
        if tag.wire_type() != WireType::Varint {
          return Err(DecodeError::WireTypeMismatch {
            field_number: 2,
            expected: VARINT,
            actual: tag.wire_type() as u8,
          });
        }
        self.end = decode_int64(buf)?;
      }
      3 => {
        if tag.wire_type() != WireType::LengthDelimited {
          return Err(DecodeError::WireTypeMismatch {
            field_number: 3,
            expected: LEN,
            actual: tag.wire_type() as u8,
          });
        }
        buffa::Message::merge_length_delimited(&mut self.timebase, buf, ctx)?;
      }
      _ => skip_field_depth(tag, buf, ctx.depth())?,
    }
    Ok(())
  }

  fn clear(&mut self) {
    *self = TimeRange::default();
  }
}

/// buffa's view contracts for a wire type that holds scalars only: it is its
/// own view, borrowing nothing, so a view decodes, merges, encodes and turns
/// back into the owned message exactly as the message itself does.
///
/// The generated view of an enclosing message decodes a field of one of
/// these types through `MessageView::decode_view_ctx` and merges a repeated
/// occurrence through `merge_into_view`, measures and writes it through
/// `ViewEncode`, and reaches an unset one through `DefaultViewInstance`; all
/// four forward to the owned `Message` impl. `merge_into_view` merges the
/// whole sub-message at once, as `Message::merge` does.
macro_rules! scalar_view {
  ($ty:ty) => {
    impl<'a> ::buffa::MessageView<'a> for $ty {
      type Owned = $ty;

      fn decode_view(buf: &'a [u8]) -> Result<Self, DecodeError> {
        <$ty as Message>::decode_from_slice(buf)
      }

      fn merge_into_view(
        &mut self,
        buf: &'a [u8],
        ctx: DecodeContext<'_>,
      ) -> Result<(), DecodeError> {
        let mut cur = buf;
        <$ty as Message>::merge(self, &mut cur, ctx)
      }

      fn merge_view_field(
        &mut self,
        tag: Tag,
        cur: &'a [u8],
        _before_tag: &'a [u8],
        ctx: DecodeContext<'_>,
      ) -> Result<&'a [u8], DecodeError> {
        let mut cur = cur;
        <$ty as Message>::merge_field(self, tag, &mut cur, ctx)?;
        Ok(cur)
      }

      fn to_owned_message(&self) -> Result<$ty, DecodeError> {
        Ok(*self)
      }
    }

    impl ::buffa::ViewEncode<'_> for $ty {
      fn compute_size(&self, cache: &mut SizeCache) -> u32 {
        <$ty as Message>::compute_size(self, cache)
      }

      fn write_to(&self, cache: &mut SizeCache, buf: &mut impl EncodeSink) {
        <$ty as Message>::write_to(self, cache, buf)
      }
    }

    impl ::buffa::DefaultViewInstance for $ty {
      fn default_view_instance<'a>() -> &'a Self
      where
        Self: 'a,
      {
        <$ty as DefaultInstance>::default_instance()
      }
    }
  };
}

scalar_view!(Timebase);
scalar_view!(Timestamp);
scalar_view!(TimeRange);

/// The ancillary module buffa's code generator looks for under an
/// `extern_path` target when a mapped type is a message field with view
/// generation enabled. Every type here holds scalars only and carries the
/// view contracts itself, so each view is the owned type.
#[doc(hidden)]
pub mod __buffa {
  pub mod view {
    // `'a` is required by buffa's extern-view convention; unused here
    // because these types are `Copy`/owned (nothing borrowed).
    pub type TimebaseView<'a> = super::super::Timebase;
    pub type TimeRangeView<'a> = super::super::TimeRange;
    pub type TimestampView<'a> = super::super::Timestamp;
  }
}

#[cfg(test)]
mod tests;
