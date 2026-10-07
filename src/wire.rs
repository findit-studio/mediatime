//! The `mediatime.v1` protobuf package as the wire carries it, for buffa's
//! `extern_path` — the mapping that keeps protobuf's merge semantics.
//!
//! `extern_path(".mediatime.v1", "::mediatime::wire")` maps the package
//! here. [`Timebase`] and [`Timestamp`] are their own wire faces: every value
//! a field of either can carry is one the type holds. A range is different.
//! Its order relates two fields, and protobuf lets a message field arrive in
//! several occurrences that are merged — `{start: 100}` and then
//! `{end: 200}` is `[100, 200)` — so the first occurrence alone is an
//! inverted range, and only the enclosing message knows when the last one has
//! arrived. buffa has no hook that runs once it has, so the order cannot be
//! judged while the field decodes. [`TimeRange`] here is the field exactly as
//! merged, with no order between its endpoints; the domain
//! [`crate::TimeRange`] is reached by a checked conversion once the enclosing
//! message is decoded:
//!
//! ```
//! use buffa::Message;
//! use mediatime::{Timebase, wire};
//!
//! let range = mediatime::TimeRange::new(100, 200, Timebase::MILLIS);
//! let bytes = wire::TimeRange::from(range).encode_to_vec();
//! let decoded = wire::TimeRange::decode_from_slice(&bytes).unwrap();
//! assert_eq!(mediatime::TimeRange::try_from(decoded), Ok(range));
//! ```
//!
//! Mapping the package onto the crate root instead (`"::mediatime"`) decodes
//! straight into the domain types, through [`crate::TimeRange`]'s own
//! `Message` impl. That impl cannot see the enclosing message either, so it
//! judges every merge on its own: it never holds an inverted range, and it
//! refuses a split field whose first part is one.

use core::fmt;

use ::buffa::{
  DecodeContext, DecodeError, DefaultInstance, EncodeSink, Message, SizeCache,
  bytes::Buf,
  encoding::{Tag, WireType, encode_varint, skip_field_depth, varint_len},
  types::{decode_int64, encode_int64, int64_encoded_len},
};

pub use crate::{Timebase, Timestamp};

/// A `mediatime.v1.TimeRange` message as merged from the wire: two endpoints
/// and a timebase, with no order between the endpoints.
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

impl From<crate::TimeRange> for TimeRange {
  #[cfg_attr(not(tarpaulin), inline(always))]
  fn from(range: crate::TimeRange) -> Self {
    Self {
      start: range.start_pts(),
      end: range.end_pts(),
      timebase: range.timebase(),
    }
  }
}

/// Returned when a wire range's `end` precedes its `start`, which
/// [`crate::TimeRange`] cannot hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvertedRange(());

impl fmt::Display for InvertedRange {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("time range end precedes its start")
  }
}

impl core::error::Error for InvertedRange {}

/// The checked conversion: the wire range as a [`crate::TimeRange`], or
/// [`InvertedRange`] if its `end` precedes its `start`.
impl TryFrom<TimeRange> for crate::TimeRange {
  type Error = InvertedRange;

  #[cfg_attr(not(tarpaulin), inline(always))]
  fn try_from(range: TimeRange) -> Result<Self, InvertedRange> {
    Self::try_new(range.start, range.end, range.timebase).ok_or(InvertedRange(()))
  }
}

const VARINT: u8 = WireType::Varint as u8;
const LEN: u8 = WireType::LengthDelimited as u8;

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

/// The ancillary module buffa's code generator looks for under an
/// `extern_path` target when a mapped type is a message field with view
/// generation enabled; every type here holds scalars only, so each view is
/// the owned type itself.
#[doc(hidden)]
pub mod __buffa {
  pub mod view {
    // `'a` is required by buffa's extern-view convention; unused here
    // because these types are `Copy`/owned (nothing borrowed).
    pub type TimebaseView<'a> = crate::Timebase;
    pub type TimeRangeView<'a> = super::super::TimeRange;
    pub type TimestampView<'a> = crate::Timestamp;
  }
}

#[cfg(test)]
mod tests;
