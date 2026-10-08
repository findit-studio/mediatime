# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-10-08

### Added

- `Rounding { Nearest, Floor, Ceil, Exact }` (`#[non_exhaustive]`) — which
  way a value that falls between two ticks goes. `Nearest` is FFmpeg's
  `AV_ROUND_NEAR_INF`, the rule every rescale that names none keeps;
  `Floor` and `Ceil` are directions on the number line at either sign;
  `Exact` refuses a value between ticks.
- Directed rescales: `Timebase::checked_rescale_with(pts, to, rounding)`,
  and `checked_rescale_with(target, rounding)` on `Timestamp`,
  `SignedDuration` and `Duration` (the last over its full `u64` range).
  `checked_rescale` is unchanged; a property pins that naming `Nearest`
  reproduces it for every input.
- `Timebase::checked_rescale_exact(pts, to)` — the exact-or-none rescale:
  `Some` only when the instant lands on a tick of `to`, and an answer
  rescales back exactly.
- `ExactSeconds` — an exact, signed number of seconds (`i128` over a
  positive `i128`, in lowest terms): the sum across timebases that does not
  round, where `SignedDuration::checked_add` rescales its right operand to
  the nearest tick. `from_timestamp`/`from_signed_duration`/`from_duration`
  fold in exactly; `checked_add`/`checked_sub` stay exact or answer `None`;
  `checked_to_timestamp`/`checked_to_signed_duration`/`checked_to_duration`
  read the total back once, by a `Rounding` — by long division, so every
  count that fits answers however large the denominators behind it. `Ord`
  compares by Euclid's algorithm, so denominators whose cross product leaves
  `i128` still order exactly.
- `TimeRange::{contains_instant, contains, overlaps, within, before,
  after}` — ingraph's `MediaTimeRangeFilter` algebra in memory, operator for
  operator, compared exactly across timebases. The type's docs carry the
  table and every degenerate case: an instant at `start` and at `end`,
  abutting ranges, zero-length ranges, a range in a degenerate timebase.
- `Timestamp::parse_seconds(text, timebase, rounding)` — decimal seconds
  read exactly, as integers rather than through a float, and counted in
  `timebase` by the rounding named. `ParseSecondsError` (`#[non_exhaustive]`:
  `NotDecimal`, `BetweenTicks`, `OutOfRange`, `DegenerateTimebase`) says
  why a text was refused — `BetweenTicks` is the refusal by name under
  `Rounding::Exact`.
- `Rate::as_f64` — the double nearest a rate, for the places that need a
  float; lossy, and documented as such.
- `mediatime::wire` (`buffa` feature) — the `mediatime.v1` package as
  protobuf reads it, for `extern_path(".mediatime.v1", "::mediatime::wire")`.
  `wire::Timebase`, `wire::Timestamp` and `wire::TimeRange` are plain wire
  types that start from protobuf's zero state and keep what they read: a
  proto3 encoder's `0/3`, written as the denominator alone, reads back as
  `0/3`, and a range field split over several occurrences merges.
  `TryFrom<wire::X> for X` is the checked conversion, naming what fails in
  `wire::ConversionError`; `From<X>` is the total one. The types write
  proto3's canonical form — a scalar only when it is not zero, a nested
  timebase only when present — byte for byte what buffa's generated code
  writes for the same messages, so a decode and re-encode merges as the
  original does. Each wire type carries buffa's view contracts, so
  generated code holding a
  `.mediatime.v1` field compiles under buffa's default view generation — a
  test compiles buffa-codegen 0.9's output for such a message.

### Changed

- **Breaking:** a `TimeRange`'s endpoints are ordered by construction
  everywhere. `with_start`, `with_end`, `set_start` and `set_end` are
  removed: they assigned without checking, so a safe call could build a
  range whose `end` precedes its `start` — one every other road refuses,
  and one this version's own `buffa` decoder would not read back. In their
  place, `try_with_start`/`try_with_end`/`try_set_start`/`try_set_end`
  answer `Err(InvertedRange)` (a new error type, which serde's refusal now
  uses too) and leave the range as it was, and `with_bounds`/`set_bounds`
  move both ends at once, panicking on inverted bounds as `TimeRange::new`
  does. `TimeRange::duration` no longer has a panic path.
- **Breaking:** `Timebase`, `Timestamp` and `TimeRange` no longer implement
  buffa's `Message` or `DefaultInstance`, and the crate root's
  `__buffa::view` aliases are gone: the `mediatime.v1` package maps onto
  `::mediatime::wire` (`extern_path(".mediatime.v1", "::mediatime::wire")`),
  and a generated container holds wire values, converted at the edge with
  `TryFrom<wire::X> for X` and `From<X> for wire::X`. A domain type has no
  protobuf zero state, and buffa builds a mapped value without decoding it
  on several roads — an omitted map value, an unset field's default
  instance, an element or a message before its merge — so the domain
  mapping invented values there (an omitted map value read as
  `Timestamp(0 @ 1/1)`, which a re-encode then wrote out) that no codec of
  its own could refuse. The wire types hold the zero message on those
  roads, and the conversion refuses it by name (`MissingTimebase`).
- `Rate`'s `FromStr` also reads a whole number of events per second:
  `"25"` is `25/1`, through `Rate::try_hz`. A decimal rate stays refused
  (`23.976` is not `24000/1001`), and `Timebase`'s door still takes no bare
  number. `ParseRateError`'s message names the new arm.

### Fixed

- An inverted `TimeRange` no longer comes out of buffa. 0.4.0's decoder
  admitted one from a peer, after which `duration()` panicked; the domain
  type now has no decoder at all, and `TryFrom<wire::TimeRange>` refuses an
  inverted range by name (`InvertedRange`), whether it arrived whole or
  split over occurrences that `wire` merges as protobuf does.
- Documentation: a zero `Timebase` numerator stays legal — `0/1` is
  libavformat's "undeclared" timebase, and every reader accepts what the
  constructors build — and `Timebase`'s docs now say what every road does
  with one. `Timestamp::duration` states its answer for a degenerate
  timebase (zero, for any count).

## [0.4.0] - 2026-08-27

### Added

- `Duration` — the unsigned counterpart to `SignedDuration`: `{ ticks: u64,
  timebase }` for a media duration that is never negative, with the same
  named ladder minus the sign-only ops. `checked_`/`saturating_` ×
  `add`/`sub` (subtraction refuses or clamps to zero instead of going
  negative — `u64::checked_sub`/`saturating_sub`'s posture, not a
  pathological edge), `cmp_semantic`, and `rescale_to`/`checked_rescale_to`.
  `is_zero` alone survives from `SignedDuration`'s
  `is_negative`/`is_positive`/`is_zero` trio (`is_negative` has no answer but
  `false`, `is_positive` is exactly `!is_zero()`); `neg`/`abs` are dropped
  outright, an unsigned count having no opposite or magnitude to give.
  Deliberately no `Ord`, for `SignedDuration`'s reason.
- Conversions both ways with `core::time::Duration` — imported internally as
  `StdDuration`, freeing the bare `Duration` name for the new type (the
  crate's own call sites for the standard type were mechanically
  repointed): `checked_from_std`/`saturating_from_std`/`checked_to_std`/
  `saturating_to_std`. The timebase conversion math grows a private `u64`
  rung for this (`Timebase::tick_nanos_unsigned` beside `tick_nanos`, and a
  `rescaled_unsigned`/`checked_rescale_unsigned`/`saturating_rescale_unsigned`/
  `checked_recount_unsigned`/`saturating_recount_unsigned` family beside
  their `i64` counterparts, all crate-private) rather than `Duration` slotting
  into `Timebase`'s public, `i64`-bound `checked_duration_to_pts` family: a
  `Duration` counts up to `u64::MAX` ticks, twice that family's reach, and
  reusing it would have silently halved a `Duration`'s usable range.
  `Timebase`'s public surface is unchanged.
- Conversions both ways with `SignedDuration` — the checked sign transition:
  `checked_from_signed`/`saturating_from_signed` (refuses or clamps a
  backward span to zero) and `checked_to_signed`/`saturating_to_signed`
  (refuses or clamps past `i64::MAX`, since a `Duration` can count twice as
  far).
- `Display`/`FromStr` for `Duration`: the same exact `ticks @ num/den` form
  `SignedDuration` writes, minus the sign. Its own parse error,
  `ParseDurationError`.
- `serde`/`quickcheck`/`arbitrary` treatments for `Duration`, mirroring
  `SignedDuration`'s: a plain field derive for `serde` (the count has no
  invariant to enforce, so there is nothing for a manual impl to validate);
  full-`u64`-range generators for `quickcheck`/`arbitrary`, unfiltered, for
  the same reason. No `buffa` treatment — `SignedDuration` has none either.

## [0.3.0]

### Added

- `SignedDuration` — the vector to `Timestamp`'s point: `{ ticks: i64, timebase }`
  with the full named ladder (`checked_`/`saturating_` × `neg`/`abs`/`add`/`sub`),
  `cmp_semantic`, and the `rescale_to`/`checked_rescale_to` pair. Deliberately no
  `Ord`: length ordering is asked for by name — `sort_by(SignedDuration::cmp_semantic)`
  — because the structural order a derive would give sorts `2 @ 1/1` (two seconds)
  ahead of `1000 @ 1/1000` (one second).
- `Rate` — the other reading of a rational, where the value *is* the rate
  (`30000/1001` fps stores `30000/1001`): `hz`/`fps` constructors with `try_`
  twins, the reciprocal layer `to_timebase`/`from_timebase`
  (+ `checked_` twins), eight `FPS_*` constants with `from_name`/
  `well_known_name`, and the frame arithmetic that used to live on `Timebase`:
  `checked_frames_to_duration`/`saturating_frames_to_duration`.
- `Timestamp::signed_duration_since`/`checked_signed_duration_since` and the
  four signed rungs `checked_add_signed`/`saturating_add_signed`/
  `checked_sub_signed`/`saturating_sub_signed`.
- `Display`/`FromStr` for the two new types: `SignedDuration` renders the exact
  form (`-1500 @ 1/1000`), `Rate` the numeric form and additionally parses its
  roster names. Each has its own parse error (`ParseSignedDurationError`,
  `ParseRateError`); the two rosters are deliberately disjoint
  (`"MILLIS".parse::<Rate>()` errors).
- `Display` for `Timebase`, `Timestamp` and `TimeRange`.
- The `Timebase` roster grows to 25 named constants: the audio sample-rate
  family (`HZ_8K` … `HZ_192K`, fourteen members) and the frame-interval family
  completed to eight (`VIDEO_30`, `PAL_50`, `NTSC_60`, `VIDEO_60` join
  `NTSC_FILM`/`NTSC_VIDEO`/`FILM_24`/`PAL_25`), with `from_name`/
  `well_known_name` and a test-pinned bijection against the `Rate` roster. One
  value, one name — no aliases (Matroska's millisecond base is `MILLIS`).
- Name lookups (`from_name`, and `FromStr` where it reads names) accept
  ASCII-case-insensitively; the canonical spelling is what `well_known_name`
  returns.

### Changed

- **Breaking:** the bare-name arithmetic is gone. `Timebase::rescale_pts`,
  `Timebase::rescale` and `Timebase::duration_to_pts` are replaced by the
  named ladder: `checked_rescale`/`saturating_rescale` and
  `checked_duration_to_pts`/`saturating_duration_to_pts` — every lossy
  operation spells its overflow posture.
- **Breaking:** rounding is round-to-nearest, ties away from zero — FFmpeg's
  `AV_ROUND_NEAR_INF`, which is what `av_rescale_q` actually defaults to. The
  old docs claimed truncation matched `av_rescale_q`; they were wrong, and the
  whole ladder (rescale, duration conversion, and `Rate`'s frame arithmetic)
  now rounds one way.
- **Breaking:** a degenerate timebase (`num == 0`) now panics on both
  saturating rungs (`saturating_rescale` already did;
  `saturating_duration_to_pts` answered `0`) — `i64::saturating_div`'s
  precedent: saturation is a posture toward overflow, and a degenerate base
  has no quotient to clamp. `Timestamp::saturating_add_duration`/
  `saturating_sub_duration` panic accordingly instead of silently not moving.
- **Breaking:** `Timestamp`'s semantic comparison guards its same-timebase
  fast path against degenerate timebases. Before the guard, `Eq` was not
  transitive (`1 @ 0/1 == 0 @ 1/1` and `2 @ 0/1 == 0 @ 1/1`, yet
  `1 @ 0/1 != 2 @ 0/1`), which `BTreeMap`/`sort` rely on; every instant of a
  degenerate timebase now compares equal through the cross-multiply, agreeing
  with `Hash`.

## [0.2.0]

### Changed

- **Breaking:** `Timebase`'s numerator and denominator are now signed —
  `num: u32 → i32` and `den: NonZeroU32 → NonZeroI32`. FFmpeg's `AVRational`
  is a pair of C `int`s, so a `u32` numerator or denominator above `i32::MAX`
  was representable but could not round-trip into an `AVRational` — usable in
  Rust, unusable at the boundary with the decoder library this crate exists to
  serve. `i32` is also a native `INTEGER` on PostgreSQL, MySQL and SQLite,
  whereas `sqlx` has no `Type<Postgres>`/`Encode<Postgres>` for `u32` at all.
  `new`, `try_new`, `num`, `den`, `with_num`, `with_den`, `set_num` and
  `set_den` all change signature.
- **Breaking:** `Timebase::new` now panics on `num < 0` or `den < 0`.
  `NonZeroI32` carries only the non-zero half of what `NonZeroU32` guaranteed,
  so the sign half moved into the constructor; the setters route through it so
  there is one enforcement site. A zero numerator remains legal (a degenerate
  timebase, still not a valid rescale target).
- Bump `buffa` dependency from `0.8` to `0.9`. buffa 0.9 replaced the
  `BufMut` bound on `Message::write_to` with its new `EncodeSink` trait, so
  the three hand-written `write_to` impls change one parameter type; every
  `BufMut` implementor is an `EncodeSink` through a blanket impl, so callers
  passing `Vec<u8>`/`BytesMut` are unaffected. Consumers that bump to
  `buffa 0.9` must also bump their `mediatime` floor to `0.2.0` so a single
  `buffa` version stays in the dependency graph.
- Bump `quickcheck-richderive` dependency from `0.3` to `0.4`. The derive's
  0.4.0 is a one-dependency release — `syn 2 → syn 3` — forced by syn 3
  replacing `Signature::unsafety: Option<Token![unsafe]>` with the tri-state
  `Signature::safety` that Rust 2024's `unsafe extern` needs. It changes no
  emitted token and no diagnostic, so the three derived
  `impl quickcheck::Arbitrary` blocks on `Timebase`, `Timestamp` and
  `TimeRange` expand byte-for-byte as before; the bump is confined to the
  optional `quickcheck` feature and moves no public API, no wire format and
  no runtime behaviour. It does drop `syn 2` from the normal dependency
  graph, leaving `syn 3` as the only `syn` a consumer compiles for
  mediatime itself.

### Added

- `Timebase::try_new(num, den) -> Option<Self>` — fallible counterpart to
  `Timebase::new`, mirroring `TimeRange::try_new`.
- `Display` for `Timebase`, `Timestamp` and `TimeRange`, each with a readable
  default form and an exact alternate form under `{:#}`:

  | type | `{}` | `{:#}` |
  |---|---|---|
  | `Timebase` | `1/1000` | `1/1000` |
  | `Timestamp` | `0:00:00.137` | `12345 @ 1/90000` |
  | `TimeRange` | `[0:00:01.500, 0:00:03.250)` | `[1500, 3250) @ 1/1000` |

  `Timebase` prints the form proposed in the request, unreduced — the timebase
  a stream declared is the one worth reading in a log, and `2/4` would
  otherwise be indistinguishable from `1/2`.

  `Timestamp` diverges from `video-rs`, which prints the unreduced rational
  (`12345/90000 secs`) in this position. Readable log messages were the point
  of the request, and that form makes the reader do the division; the rational
  stays available under `{:#}`. Hours are unpadded and unbounded
  (`123:45:06.789`), minutes and seconds are two digits, milliseconds three,
  and a negative PTS signs the whole rendering (`-0:00:01.500`) since pre-roll
  and edit lists produce one. The value is truncated toward zero at
  millisecond resolution, as `rescale_pts` truncates, so `{}` is lossy in both
  precision and timebase — `{:#}` and the derived `Debug` are the exact forms.

  `TimeRange` renders `[…)` because the interval is half-open, so the notation
  carries the semantics the type documents; `{:#}` names the shared timebase
  once, after both endpoints.

  Nothing allocates: the impls write directly into the `Formatter`, so the
  crate remains `no_std` with no `alloc`. One consequence is documented on each
  impl — width and alignment flags (`{:>12}`) are ignored, because honouring
  them means measuring the finished string and there is no buffer to build one
  in.

### Fixed

- Deserializing a `Timebase` can no longer produce a value the constructor
  would reject. serde's derive assigns fields directly, and the field types no
  longer make a negative numerator or denominator unrepresentable, so both
  fields are validated on the way in.

### Wire compatibility

- **The buffa wire format is unchanged.** The two `Timebase` fields move from
  protobuf `uint32` to `int32`, which is the same plain (non-ZigZag) varint for
  every value a `Timebase` can hold; bytes encoded by earlier versions still
  decode to the same value, and a golden-bytes test pins this. Values above
  `i32::MAX` written by an older peer decode to the smallest legal value
  rather than panicking — they were never representable in the new type.
- **The buffa 0.9 encoder swap is byte-for-byte transparent** as well: the tag,
  varint and length-delimited encoders emit the same bytes through `EncodeSink`
  as they did through `BufMut`, and bytes written under buffa 0.6/0.7/0.8 still
  decode.
- **The serde representation is unchanged** for all in-range values: the
  `numerator`/`denominator` field names and their JSON number encoding are
  untouched.

## [0.1.8] — 2026-06-02

### Changed

- Bump `buffa` dependency from `0.6` to `0.7`. Pure version bump — the
  buffa 0.6 → 0.7 breakers (`OwnedView::Deref` removal and the
  `use_bytes_type` extension to `map<K, bytes>` values) don't touch
  mediatime, and the `DefaultInstance` + `Message` impls on `Timebase`,
  `Timestamp`, and `TimeRange` carry over byte-for-byte. The wire format
  is unchanged. Consumers (e.g. mediaschema) that bump to `buffa 0.7`
  must also bump their `mediatime` floor to `0.1.8` so a single `buffa`
  version stays in the dependency graph.

## [0.1.5] — April 23, 2026

### Added

- `Timestamp::duration() -> Option<Duration>` — [`Duration`] from PTS zero in
  the timestamp's own timebase. Returns `None` for negative PTS (pre-roll /
  edit-list cases that have no [`Duration`] representation).
- `TimeRange::rescale_to(target: Timebase) -> Self` — rescales both endpoints
  to a new timebase (parallel to `Timestamp::rescale_to`). Monotonic, so the
  `start <= end` invariant is preserved.
- `TimeRange::total_pts() -> i64` — span in PTS units (`end - start`);
  saturates at `i64::MAX` for pathological `i64::MIN..i64::MAX` inputs.
- `TimeRange::try_new(start, end, timebase) -> Option<Self>` — fallible
  counterpart to `TimeRange::new`; returns `None` instead of panicking when
  `end < start`. Degenerate instant ranges (`start == end`) are accepted.

## [0.1.0] — April 17, 2026

Initial public release. First-cut API — expect minor refinements before 1.0.

### Added

- `Timebase` — rational `num/den` (`u32` numerator, `NonZeroU32` denominator).
  Mirrors FFmpeg's `AVRational`. Supports value-based equality, ordering, and
  hashing (reduced-form rational), so `1/2 == 2/4 == 3/6` and all three hash
  identically.
- `Timestamp` — integer PTS (`i64`) tagged with a `Timebase`. Semantic
  comparison across different timebases via 128-bit cross-multiplication —
  no rounding, no division.
- `TimeRange` — half-open `[start, end)` interval sharing a `Timebase`, with
  `start()` / `end()` as `Timestamp`, `duration()`, and clamped linear
  `interpolate(t)` for midpoint / bias placement.
- Timebase utilities: `rescale_pts` (FFmpeg's `av_rescale_q`), `rescale`,
  `frames_to_duration`, `duration_to_pts`, `num`/`den` accessors, `with_*`
  consuming builders and `set_*` in-place setters.
- Timestamp utilities: `pts`/`timebase` accessors, `with_pts`/`set_pts`,
  `rescale_to`, `saturating_sub_duration`, `duration_since`, `cmp_semantic`
  (const-fn form of `Ord::cmp`).
- TimeRange utilities: `new`, `instant`, `start_pts`/`end_pts`/`timebase`
  accessors, `with_*`/`set_*` setters for both endpoints, `is_instant`.
- `const fn` across the whole public surface — every constructor, accessor,
  and setter can be evaluated in a `const` context.
- `#![no_std]` always, zero dependencies. No allocation anywhere — every
  public type is `Copy`.

### Behavior

- All comparisons between types are **semantic**, not structural: two
  `Timestamp`s representing the same instant in different timebases are
  `Eq`, `Ord::Equal`, and hash the same. Use this directly as a `HashMap` or
  `BTreeMap` key without worrying about canonicalization.
- Cross-timebase arithmetic (rescaling, `duration_since`) uses 128-bit
  intermediates throughout — exact for any `u32`×`u32` timebase combined
  with any `i64` PTS in the real-video range.
- `rescale_pts` rounds toward zero, matching `av_rescale_q` with default
  rounding. Saturating variants are not provided yet — overflow in
  `duration_to_pts` is clamped to `i64::MAX`.

### Testing

- 100% line coverage on `src/lib.rs` under
  `cargo tarpaulin --all-features --run-types tests --run-types doctests`.
- Criterion bench (`cargo bench --bench gcd`) for the internal GCD helpers
  used by `Hash`; ships both Euclidean and binary variants for comparison.
