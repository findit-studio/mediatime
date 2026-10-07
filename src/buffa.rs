//! How the mediatime types meet buffa, behind the `buffa` feature.
//!
//! The `mediatime.v1` package —
//!
//! ```text
//! Timebase  { int32 num = 1;   int32 den = 2; }
//! TimeRange { int64 start = 1; int64 end = 2; Timebase timebase = 3; }
//! Timestamp { int64 pts = 1;   Timebase timebase = 2; }
//! ```
//!
//! — maps onto [`crate::wire`], and only there:
//! `extern_path(".mediatime.v1", "::mediatime::wire")`. The wire types hold a
//! message exactly as protobuf reads it, from its zero state, and carry
//! `buffa::Message` and the view contracts; the domain types carry neither.
//!
//! A domain type has no protobuf zero state — a `Timebase` cannot be `0/0`, a
//! `Timestamp` cannot lack a timebase — and buffa builds a mapped value
//! without decoding it on several roads: an omitted map value is
//! `Default::default()`, inserted as it is; an unset singular field reads as
//! `DefaultInstance::default_instance()`; `MessageField::get_or_insert_default`,
//! a repeated element and `Message::decode` start from `Default::default()`
//! before they merge; an unset view field reads as
//! `DefaultViewInstance::default_view_instance()`. A domain type there would
//! have to invent a value — `Timestamp(0 @ 1/1)` for a map value no peer sent
//! — and no codec of its own could refuse what buffa never asks it to read.
//!
//! The edge is the checked conversion, once the enclosing message is decoded:
//! `TryFrom<wire::X> for X`, which names what a value fails
//! (`wire::ConversionError`), and `From<X> for wire::X` back. A map entry
//! whose value was omitted holds the wire zero value, which the conversion
//! refuses as `MissingTimebase`: read as it arrived, never filled in.
//!
//! `Timebase`'s fields were `uint32` before the type became signed. Protobuf's
//! `int32` and `uint32` are the same plain (non-ZigZag) varint for values a
//! `Timebase` can hold — both are non-negative and at most `i32::MAX` — so the
//! bytes are unchanged in both directions. `sint32` would have been the
//! silent break: ZigZag re-encodes every value.
