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
    DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
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

/// A downstream message holding each `mediatime.v1` type once, and a range
/// repeated.
fn clip_proto() -> FileDescriptorProto {
  let one = Label::LABEL_OPTIONAL;
  let range = Some(".mediatime.v1.TimeRange");
  FileDescriptorProto {
    name: Some("mediatime05/test/clip.proto".into()),
    package: Some("mediatime05.test".into()),
    syntax: Some("proto3".into()),
    dependency: vec!["mediatime/v1/mediatime.proto".into()],
    message_type: vec![message(
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
      ],
    )],
    ..Default::default()
  }
}

/// `buffa-codegen`'s output for `clip.proto`, with buffa's defaults and the
/// package mapped onto `::mediatime::wire`.
fn generated() -> Vec<(String, String)> {
  let mut config = CodeGenConfig::default();
  config.extern_paths = vec![(".mediatime.v1".into(), "::mediatime::wire".into())];
  assert!(
    config.generate_views,
    "views are buffa's default, and under test"
  );
  buffa_codegen::generate(
    &[mediatime_v1(), clip_proto()],
    &["mediatime05/test/clip.proto".into()],
    &config,
  )
  .expect("codegen")
  .into_iter()
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
    3,
    "the owned file, its views, and the module tree"
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
