//! `buffa::Message` implementations for the mediatime types, behind the
//! `buffa` feature. Used via `extern_path` from buffa-generated crates.
//!
//! Wire format (clean redesign — no compatibility with findit-proto's
//! hand-rolled encoding is required):
//!   Timebase  { int32  num = 1;  int32  den = 2; }
//!   TimeRange { int64  start = 1; int64  end = 2; Timebase timebase = 3; }
//!   Timestamp { int64  pts = 1;  Timebase timebase = 2; }
//!
//! Each domain type is written and read through its [`crate::wire`] twin.
//! Writing is the wire type's encoding of the value. Reading takes one
//! message into the wire type from protobuf's zero state — an absent scalar
//! is zero, an absent timebase is absent — and converts it with the checked
//! conversion: a zero numerator is read as written, the degenerate timebase,
//! while a zero or negative denominator, a negative numerator, a missing
//! timebase and a range whose `end` precedes its `start` are refused with
//! [`DecodeError::Custom`], naming which. Nothing is repaired: a malformed
//! field has no honest repair, and a value invented here would be
//! indistinguishable downstream from one the peer sent.
//!
//! A domain type holds only valid values and has no zero state to merge
//! into, so a read *replaces* the value it lands in — whole on success,
//! untouched on a refusal or any other decode error. A field split over
//! several occurrences of an enclosing message is therefore read one
//! occurrence at a time: an occurrence that is not a whole valid value is
//! refused, and a later whole one replaces an earlier. [`crate::wire`] is the
//! mapping that merges occurrences as protobuf does.
//!
//! `Timebase`'s fields were `uint32` before the type became signed. Protobuf's
//! `int32` and `uint32` are the same plain (non-ZigZag) varint for values a
//! `Timebase` can hold — both are non-negative and at most `i32::MAX` — so the
//! bytes are unchanged in both directions. `sint32` would have been the
//! silent break: ZigZag re-encodes every value.

use ::buffa::{
  DecodeContext, DecodeError, DefaultInstance, EncodeSink, Message, SizeCache, bytes::Buf,
  encoding::Tag,
};

use crate::{TimeRange, Timebase, Timestamp, wire};

/// One message read by `read` into the wire type `W`, from protobuf's zero
/// state, and converted into `D` — or the refusal the conversion names.
fn read_whole<W, D>(read: impl FnOnce(&mut W) -> Result<(), DecodeError>) -> Result<D, DecodeError>
where
  W: Default,
  D: TryFrom<W, Error = wire::ConversionError>,
{
  let mut message = W::default();
  read(&mut message)?;
  D::try_from(message).map_err(|refusal| DecodeError::Custom(refusal.reason()))
}

/// A domain type's `Message` impl: its wire twin's encoding, and its wire
/// twin's decoding converted, replacing the value read into — see the module
/// docs.
macro_rules! domain_message {
  ($ty:ident) => {
    impl DefaultInstance for $ty {
      fn default_instance() -> &'static Self {
        static VALUE: buffa::__private::OnceBox<$ty> = buffa::__private::OnceBox::new();
        VALUE.get_or_init(|| buffa::alloc::boxed::Box::new(<$ty>::default()))
      }
    }

    impl Message for $ty {
      fn compute_size(&self, cache: &mut SizeCache) -> u32 {
        wire::$ty::from(*self).compute_size(cache)
      }

      fn write_to(&self, cache: &mut SizeCache, buf: &mut impl EncodeSink) {
        wire::$ty::from(*self).write_to(cache, buf);
      }

      // A field read on its own — the loops below never call this — has to
      // make a whole value by itself.
      fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        ctx: DecodeContext<'_>,
      ) -> Result<(), DecodeError> {
        *self = read_whole::<wire::$ty, _>(|message| message.merge_field(tag, buf, ctx))?;
        Ok(())
      }

      fn merge_to_limit(
        &mut self,
        buf: &mut impl Buf,
        ctx: DecodeContext<'_>,
        limit: usize,
      ) -> Result<(), DecodeError> {
        *self = read_whole::<wire::$ty, _>(|message| {
          message.merge_to_limit(buf, ctx, limit)?;
          // The loop stops once the buffer is at or past `limit`; past it,
          // the last field consumed bytes that are not this message's. The
          // caller reports that as `UnexpectedEof` — and the value must not
          // have moved by then.
          if buf.remaining() == limit {
            Ok(())
          } else {
            Err(DecodeError::UnexpectedEof)
          }
        })?;
        Ok(())
      }

      fn merge_group(
        &mut self,
        buf: &mut impl Buf,
        ctx: DecodeContext<'_>,
        field_number: u32,
      ) -> Result<(), DecodeError> {
        *self = read_whole::<wire::$ty, _>(|message| message.merge_group(buf, ctx, field_number))?;
        Ok(())
      }

      fn clear(&mut self) {
        *self = Self::default();
      }
    }
  };
}

domain_message!(Timebase);
domain_message!(Timestamp);
domain_message!(TimeRange);

#[cfg(test)]
mod tests;
