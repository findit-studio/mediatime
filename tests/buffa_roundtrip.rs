#![cfg(feature = "buffa")]

//! Each domain value, through its `wire` twin and back: the one road from
//! the domain types to buffa's bytes.

use buffa::Message;
use core::num::NonZeroI32;
use mediatime::{TimeRange, Timebase, Timestamp, wire};

fn nz(n: i32) -> NonZeroI32 {
  NonZeroI32::new(n).unwrap()
}

#[test]
fn timebase_roundtrips() {
  for tb in [
    Timebase::new(30000, nz(1001)),
    Timebase::new(0, nz(1)),
    Timebase::new(1, nz(48000)),
  ] {
    let bytes = wire::Timebase::from(tb).encode_to_vec();
    let back = wire::Timebase::decode_from_slice(&bytes).expect("decode");
    let back = Timebase::try_from(back).expect("a timebase");
    assert_eq!((back.num(), back.den()), (tb.num(), tb.den()), "as written");
  }
}

#[test]
fn timerange_roundtrips() {
  let tb = Timebase::new(1, nz(90000));
  for tr in [
    TimeRange::new(0, 0, tb),
    TimeRange::new(100, 250, tb),
    TimeRange::new(-5, 5, Timebase::new(30000, nz(1001))),
  ] {
    let bytes = wire::TimeRange::from(tr).encode_to_vec();
    let back = wire::TimeRange::decode_from_slice(&bytes).expect("decode");
    assert_eq!(
      TimeRange::try_from(back),
      Ok(tr),
      "TimeRange round-trip failed"
    );
  }
}

#[test]
fn timestamp_roundtrips() {
  let tb = Timebase::new(1, nz(1000));
  for ts in [
    Timestamp::new(0, tb),
    Timestamp::new(123456, tb),
    Timestamp::new(-99, Timebase::new(24000, nz(1001))),
  ] {
    let bytes = wire::Timestamp::from(ts).encode_to_vec();
    let back = wire::Timestamp::decode_from_slice(&bytes).expect("decode");
    assert_eq!(
      Timestamp::try_from(back),
      Ok(ts),
      "Timestamp round-trip failed"
    );
  }
}
