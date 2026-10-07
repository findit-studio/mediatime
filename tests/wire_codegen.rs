//! A message buffa generates around `.mediatime.v1` fields, with the package
//! mapped onto `::mediatime::wire` and buffa's default configuration — views
//! included — compiled here from the generator's own output.
//!
//! `tests/codegen/` holds that output as `buffa-codegen` writes it for the
//! descriptors below, and `the_checked_in_code_is_what_buffa_codegen_writes`
//! holds it there: a drift fails, and `MEDIATIME_BLESS=1` rewrites it. The
//! `clip` module below is the code itself, so this file compiles only while
//! every `wire` type satisfies what the generated code asks of it.

#![cfg(feature = "buffa")]

use core::num::NonZeroI32;

use buffa::{Message, MessageField, MessageView};
use buffa_codegen::{
  CodeGenConfig,
  generated::descriptor::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorProto, MessageOptions,
    field_descriptor_proto::{Label, Type},
  },
};
use mediatime::{Timebase, wire};

// The generator's code as it writes it: its own lints are not this crate's.
#[allow(
  clippy::all,
  dead_code,
  missing_docs,
  rust_2018_idioms,
  single_use_lifetimes,
  unreachable_pub,
  unused_imports
)]
mod clip {
  include!("codegen/mediatime05.test.mod.rs");
}

// buffa's own generated types for `.mediatime.v1`, the reference encoder.
#[allow(
  clippy::all,
  dead_code,
  missing_docs,
  rust_2018_idioms,
  single_use_lifetimes,
  unreachable_pub,
  unused_imports
)]
mod v1 {
  include!("codegen/mediatime.v1.mod.rs");
}

use clip::{Clip, ClipView};

fn field(
  name: &str,
  number: i32,
  label: Label,
  ty: Type,
  type_name: Option<&str>,
) -> FieldDescriptorProto {
  FieldDescriptorProto {
    name: Some(name.into()),
    number: Some(number),
    label: Some(label),
    r#type: Some(ty),
    type_name: type_name.map(Into::into),
    ..Default::default()
  }
}

fn message(name: &str, fields: Vec<FieldDescriptorProto>) -> DescriptorProto {
  DescriptorProto {
    name: Some(name.into()),
    field: fields,
    ..Default::default()
  }
}

/// The `mediatime.v1` package as `src/buffa.rs` documents its wire format.
fn mediatime_v1() -> FileDescriptorProto {
  let one = Label::LABEL_OPTIONAL;
  let timebase = Some(".mediatime.v1.Timebase");
  FileDescriptorProto {
    name: Some("mediatime/v1/mediatime.proto".into()),
    package: Some("mediatime.v1".into()),
    syntax: Some("proto3".into()),
    message_type: vec![
      message(
        "Timebase",
        vec![
          field("num", 1, one, Type::TYPE_INT32, None),
          field("den", 2, one, Type::TYPE_INT32, None),
        ],
      ),
      message(
        "TimeRange",
        vec![
          field("start", 1, one, Type::TYPE_INT64, None),
          field("end", 2, one, Type::TYPE_INT64, None),
          field("timebase", 3, one, Type::TYPE_MESSAGE, timebase),
        ],
      ),
      message(
        "Timestamp",
        vec![
          field("pts", 1, one, Type::TYPE_INT64, None),
          field("timebase", 2, one, Type::TYPE_MESSAGE, timebase),
        ],
      ),
    ],
    ..Default::default()
  }
}

/// The nested entry message a `map<key, value>` field is declared through.
fn map_entry(
  name: &str,
  key: Type,
  key_type: Option<&str>,
  value: Option<&str>,
) -> DescriptorProto {
  let one = Label::LABEL_OPTIONAL;
  DescriptorProto {
    options: MessageField::some(MessageOptions {
      map_entry: Some(true),
      ..Default::default()
    }),
    ..message(
      name,
      vec![
        field("key", 1, one, key, key_type),
        field("value", 2, one, Type::TYPE_MESSAGE, value),
      ],
    )
  }
}

trait WithNested {
  fn with_nested(self, nested: DescriptorProto) -> Self;
}

impl WithNested for DescriptorProto {
  fn with_nested(mut self, nested: DescriptorProto) -> Self {
    self.nested_type.push(nested);
    self
  }
}

/// A downstream message holding each `mediatime.v1` type once, a range
/// repeated, and a map of timestamps.
fn clip_proto() -> FileDescriptorProto {
  let one = Label::LABEL_OPTIONAL;
  let range = Some(".mediatime.v1.TimeRange");
  FileDescriptorProto {
    name: Some("mediatime05/test/clip.proto".into()),
    package: Some("mediatime05.test".into()),
    syntax: Some("proto3".into()),
    dependency: vec!["mediatime/v1/mediatime.proto".into()],
    message_type: vec![
      message(
        "Clip",
        vec![
          field("range", 1, one, Type::TYPE_MESSAGE, range),
          field(
            "at",
            2,
            one,
            Type::TYPE_MESSAGE,
            Some(".mediatime.v1.Timestamp"),
          ),
          field(
            "timebase",
            3,
            one,
            Type::TYPE_MESSAGE,
            Some(".mediatime.v1.Timebase"),
          ),
          field("cuts", 4, Label::LABEL_REPEATED, Type::TYPE_MESSAGE, range),
          field(
            "marks",
            5,
            Label::LABEL_REPEATED,
            Type::TYPE_MESSAGE,
            Some(".mediatime05.test.Clip.MarksEntry"),
          ),
        ],
      )
      .with_nested(map_entry(
        "MarksEntry",
        Type::TYPE_STRING,
        None,
        Some(".mediatime.v1.Timestamp"),
      )),
    ],
    ..Default::default()
  }
}

/// `buffa-codegen`'s output, with buffa's defaults: `clip.proto` with the
/// `.mediatime.v1` package mapped onto `::mediatime::wire`, and that package
/// itself generated as buffa writes any message — the reference encoder the
/// wire types are held to.
fn generated() -> Vec<(String, String)> {
  let mut mapped = CodeGenConfig::default();
  mapped.extern_paths = vec![(".mediatime.v1".into(), "::mediatime::wire".into())];
  let plain = CodeGenConfig::default();
  assert!(
    mapped.generate_views && plain.generate_views,
    "views are buffa's default, and under test"
  );
  let clip = buffa_codegen::generate(
    &[mediatime_v1(), clip_proto()],
    &["mediatime05/test/clip.proto".into()],
    &mapped,
  );
  let v1 = buffa_codegen::generate(
    &[mediatime_v1()],
    &["mediatime/v1/mediatime.proto".into()],
    &plain,
  );
  clip
    .expect("codegen, clip.proto")
    .into_iter()
    .chain(v1.expect("codegen, mediatime.proto"))
    .map(|file| (file.name, file.content))
    .collect()
}

/// The two texts with their line endings normalized: a checkout that turns
/// LF into CRLF — git's `core.autocrlf`, on by default on Windows — changes
/// the bytes of a file but not one line of its code.
fn same_text(checked_in: &str, generated: &str) -> bool {
  checked_in.replace("\r\n", "\n") == generated.replace("\r\n", "\n")
}

#[test]
fn the_checked_in_code_is_what_buffa_codegen_writes() {
  let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/codegen");
  let files = generated();
  assert_eq!(
    files.len(),
    6,
    "for each file: its owned code, its views, its module tree"
  );
  for (name, content) in files {
    let path = dir.join(&name);
    if std::env::var_os("MEDIATIME_BLESS").is_some() {
      std::fs::write(&path, &content).unwrap();
      continue;
    }
    let checked_in = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(
      same_text(&checked_in, &content),
      "{name} is not what buffa-codegen writes today; rerun with MEDIATIME_BLESS=1"
    );
  }
}

#[test]
fn a_crlf_checkout_reads_as_the_same_code() {
  // What a Windows checkout of the LF copy holds, and a real difference.
  let lf = "fn a() {}\nfn b() {}\n";
  assert!(same_text(&lf.replace('\n', "\r\n"), lf));
  assert!(!same_text("fn a() {}\r\nfn c() {}\r\n", lf));
}

fn nz(n: i32) -> NonZeroI32 {
  NonZeroI32::new(n).unwrap()
}

fn sample() -> Clip {
  let range = mediatime::TimeRange::new(100, 200, Timebase::MILLIS);
  let at = mediatime::Timestamp::new(-90_000, Timebase::MPEG_90K);
  let still = mediatime::TimeRange::new(0, 0, Timebase::NTSC_VIDEO);
  Clip {
    range: MessageField::some(range.into()),
    at: MessageField::some(at.into()),
    timebase: MessageField::some(Timebase::new(0, nz(3)).into()),
    cuts: vec![range.into(), still.into()],
    ..Default::default()
  }
}

#[test]
fn a_generated_message_carries_every_mediatime_field() {
  let clip = sample();
  let bytes = clip.encode_to_vec();
  let back = Clip::decode_from_slice(&bytes).unwrap();
  assert!(back == clip);

  let range = mediatime::TimeRange::try_from(*back.range).unwrap();
  assert_eq!(range, mediatime::TimeRange::new(100, 200, Timebase::MILLIS));
  let zero = Timebase::try_from(*back.timebase).unwrap();
  assert_eq!((zero.num(), zero.den().get()), (0, 3));
}

#[test]
fn its_view_reads_the_same_fields() {
  let clip = sample();
  let bytes = clip.encode_to_vec();
  let view = ClipView::decode_view(&bytes).unwrap();
  assert_eq!(*view.range, *clip.range);
  assert_eq!(*view.at, *clip.at);
  assert_eq!(*view.timebase, *clip.timebase);
  assert_eq!(view.cuts.len(), 2);
  assert!(view.to_owned_message().unwrap() == clip);

  // An unset field reads as protobuf's zero message, through the view too.
  let empty = ClipView::decode_view(&[]).unwrap();
  assert_eq!(*empty.timebase, wire::Timebase { num: 0, den: 0 });
}

#[test]
fn a_split_range_field_merges_in_the_message_and_in_its_view() {
  // Field 1 twice: `{start: 100, timebase: 1/1000}`, then `{end: 200}`.
  let first = wire::TimeRange {
    start: 100,
    end: 0,
    timebase: Some(Timebase::MILLIS.into()),
  };
  let second = wire::TimeRange {
    start: 0,
    end: 200,
    timebase: Some(Timebase::MILLIS.into()),
  };
  let mut bytes = Clip {
    range: MessageField::some(first),
    ..Default::default()
  }
  .encode_to_vec();
  bytes.extend(
    Clip {
      range: MessageField::some(second),
      ..Default::default()
    }
    .encode_to_vec(),
  );

  let clip = Clip::decode_from_slice(&bytes).unwrap();
  assert_eq!((clip.range.start, clip.range.end), (100, 200));
  let view = ClipView::decode_view(&bytes).unwrap();
  assert_eq!((view.range.start, view.range.end), (100, 200));
  assert_eq!(
    mediatime::TimeRange::try_from(*view.range),
    Ok(mediatime::TimeRange::new(100, 200, Timebase::MILLIS))
  );
}

#[test]
fn the_wire_types_write_the_bytes_buffa_generates() {
  for (num, den) in [(0, 0), (0, 1), (1, 0), (1, 90_000), (-7, 3), (30_000, 1001)] {
    let reference = v1::Timebase {
      num,
      den,
      ..Default::default()
    };
    let ours = wire::Timebase { num, den }.encode_to_vec();
    assert_eq!(ours, reference.encode_to_vec(), "{num}/{den}");
  }

  // A timestamp and a range with an absent, an empty, a one-field and a
  // whole timebase.
  let reference_timebase = |num, den| v1::Timebase {
    num,
    den,
    ..Default::default()
  };
  let timebases = [
    (None, MessageField::none()),
    (
      Some(wire::Timebase { num: 0, den: 0 }),
      MessageField::some(v1::Timebase::default()),
    ),
    (
      Some(wire::Timebase { num: 0, den: 3 }),
      MessageField::some(reference_timebase(0, 3)),
    ),
    (
      Some(wire::Timebase {
        num: 1,
        den: 90_000,
      }),
      MessageField::some(reference_timebase(1, 90_000)),
    ),
  ];
  for (ours, theirs) in timebases {
    for count in [0, 5, -1] {
      let stamp = wire::Timestamp {
        pts: count,
        timebase: ours,
      };
      let reference = v1::Timestamp {
        pts: count,
        timebase: theirs.clone(),
        ..Default::default()
      };
      assert_eq!(
        stamp.encode_to_vec(),
        reference.encode_to_vec(),
        "{stamp:?}"
      );
      let range = wire::TimeRange {
        start: count,
        end: 9,
        timebase: ours,
      };
      let reference = v1::TimeRange {
        start: count,
        end: 9,
        timebase: theirs.clone(),
        ..Default::default()
      };
      assert_eq!(
        range.encode_to_vec(),
        reference.encode_to_vec(),
        "{range:?}"
      );
    }
  }
}

/// The decode limits buffa applies by default, for a merge driven by hand.
fn merged_view<V: for<'a> MessageView<'a> + Copy>(existing: V, bytes: &[u8]) -> V {
  let limit = core::cell::Cell::new(buffa::DEFAULT_UNKNOWN_FIELD_LIMIT);
  let mut view = existing;
  MessageView::merge_into_view(
    &mut view,
    bytes,
    buffa::DecodeContext::new(buffa::RECURSION_LIMIT, &limit),
  )
  .unwrap();
  view
}

#[test]
fn a_decode_and_re_encode_merges_as_the_original_does() {
  // A nested timebase that is empty, or carries its denominator alone,
  // merged into a timestamp and a range that already hold `1/90000`.
  let held = wire::Timebase {
    num: 1,
    den: 90_000,
  };
  let stamp = wire::Timestamp {
    pts: 7,
    timebase: Some(held),
  };
  let range = wire::TimeRange {
    start: 1,
    end: 2,
    timebase: Some(held),
  };
  let reference_stamp = || v1::Timestamp {
    pts: 7,
    timebase: MessageField::some(v1::Timebase {
      num: 1,
      den: 90_000,
      ..Default::default()
    }),
    ..Default::default()
  };

  for original in [&[0x12, 0x00][..], &[0x12, 0x02, 0x10, 0x03][..]] {
    let again = wire::Timestamp::decode_from_slice(original)
      .unwrap()
      .encode_to_vec();
    let owned = |bytes: &[u8]| {
      let mut merged = stamp;
      merged.merge_from_slice(bytes).unwrap();
      merged
    };
    assert_eq!(owned(original), owned(&again), "{original:?}");
    assert_eq!(merged_view(stamp, original), merged_view(stamp, &again));
    assert_eq!(owned(original), merged_view(stamp, original));

    // And it is the merge buffa's generated message makes.
    let mut reference = reference_stamp();
    reference.merge_from_slice(original).unwrap();
    let ours = owned(original).timebase.unwrap();
    assert_eq!(
      (ours.num, ours.den),
      (reference.timebase.num, reference.timebase.den)
    );
  }

  for original in [&[0x1a, 0x00][..], &[0x1a, 0x02, 0x10, 0x03][..]] {
    let again = wire::TimeRange::decode_from_slice(original)
      .unwrap()
      .encode_to_vec();
    let owned = |bytes: &[u8]| {
      let mut merged = range;
      merged.merge_from_slice(bytes).unwrap();
      merged
    };
    assert_eq!(owned(original), owned(&again), "{original:?}");
    assert_eq!(merged_view(range, original), merged_view(range, &again));
  }
}

#[test]
fn a_map_value_left_out_is_the_wire_zero_and_refused_by_name() {
  // Field 5, one entry: key "a", its value omitted. buffa inserts the value
  // without decoding it — `Default::default()` — and here that is the zero
  // message, which the conversion refuses rather than a value no peer sent.
  let omitted = [0x2a, 0x03, 0x0a, 0x01, 0x61];
  let clip = Clip::decode_from_slice(&omitted).unwrap();
  let value = clip.marks.get("a").copied().expect("the entry is there");
  assert_eq!(value, wire::Timestamp::default());
  assert_eq!(
    mediatime::Timestamp::try_from(value),
    Err(wire::ConversionError::MissingTimebase)
  );

  // Re-encoded, buffa writes every map value — here an empty one, with no
  // field filled in — and it reads back as the same entry.
  let again = clip.encode_to_vec();
  assert_eq!(again, [0x2a, 0x05, 0x0a, 0x01, 0x61, 0x12, 0x00]);
  assert!(Clip::decode_from_slice(&again).unwrap() == clip);

  // An explicitly empty value is the same value, and keeps its bytes.
  let explicit = Clip::decode_from_slice(&again).unwrap();
  assert_eq!(explicit.encode_to_vec(), again);
  assert_eq!(explicit.marks.get("a"), Some(&wire::Timestamp::default()));

  // The view inserts the same zero.
  let view = ClipView::decode_view(&omitted).unwrap();
  let (key, value) = view.marks.iter().next().expect("the entry is there");
  assert_eq!((*key, *value), ("a", wire::Timestamp::default()));
}

#[test]
fn an_unset_field_and_an_empty_element_are_the_wire_zero_and_refused_by_name() {
  // A singular field never sent reads as the default instance: the zero.
  let clip = Clip::decode_from_slice(&[]).unwrap();
  assert_eq!(*clip.at, wire::Timestamp::default());
  assert_eq!(
    mediatime::Timestamp::try_from(*clip.at),
    Err(wire::ConversionError::MissingTimebase)
  );
  assert_eq!(*clip.timebase, wire::Timebase::default());
  assert_eq!(
    Timebase::try_from(*clip.timebase),
    Err(wire::ConversionError::ZeroDenominator)
  );
  assert!(clip.encode_to_vec().is_empty(), "nothing written back");

  // A repeated range with an empty element: the zero range, kept as sent.
  let bytes = [0x22, 0x00];
  let clip = Clip::decode_from_slice(&bytes).unwrap();
  assert_eq!(clip.cuts, [wire::TimeRange::default()]);
  assert_eq!(
    mediatime::TimeRange::try_from(clip.cuts[0]),
    Err(wire::ConversionError::MissingTimebase)
  );
  assert_eq!(clip.encode_to_vec(), bytes);
  let view = ClipView::decode_view(&bytes).unwrap();
  assert_eq!(
    view.cuts.iter().copied().collect::<Vec<_>>(),
    [wire::TimeRange::default()]
  );
}
