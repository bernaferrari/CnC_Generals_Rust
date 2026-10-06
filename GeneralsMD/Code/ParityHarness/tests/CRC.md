# Common CRC regression evidence

This checks `Common/src/common/crc.rs`, the byte-oriented shift/add checksum.
It does not cover XferCRC, transport CRCs, the trace envelope's polynomial
CRC-32, or network behavior.

`crc_original.cpp` links `original_random_adapter.cpp`, which textually includes
the unchanged original `Source/Common/crc.cpp` and `RandomValue.cpp`. The driver
contains input cases and output formatting, not a replacement CRC recurrence.
It executes the original `_DEBUG` C++ implementation through the existing narrow
portability headers. The retail x86 assembly was inspected, not executed.

Original source SHA-256 pins:

- `crc.cpp`: `0d09cb31ed3d494a2479594ab06869517265c3eaafbb0ef3e463b0fb2978ea2d`
- `RandomValue.cpp`: `eec60d493daff4988579fa032f31963e276615a56e501b0c1961b00c213a2e06`

The driver requires eight-bit bytes, four-byte original/shim unsigned words,
and native little-endian representation before producing typed fixtures.
Its RNG calls use the bounded range 0..1000. It never executes the existing
adapter's 0..INT_MAX draw helper, whose signed range expression overflows.

## Regenerate and compare

From the repository root, with C++ and Rust toolchains on PATH:

```sh
crc_evidence=$(mktemp -d)
c++ -std=c++17 -O2 -Wall -Wextra -Wpedantic -D_DEBUG \
  -fsanitize=undefined -fno-sanitize-recover=all \
  -IGeneralsMD/Code/ParityHarness/shims \
  GeneralsMD/Code/ParityHarness/tests/crc_original.cpp \
  GeneralsMD/Code/ParityHarness/original_random_adapter.cpp \
  -o "$crc_evidence/crc_original"
"$crc_evidence/crc_original" > "$crc_evidence/crc_original.txt"
diff -u GeneralsRust/Code/GameEngine/Common/tests/fixtures/crc_original.txt \
  "$crc_evidence/crc_original.txt"
```

There are 11 byte/word checksums and 25 RNG checkpoints (five seeds, after
0/1/2/7/32 draws). The original driver checks every split, byte-at-a-time
accumulation, empty/null/invalid-length no-ops, clear/reuse, and non-mutating
seed CRC reads. The checked-in fixture is its exact stdout.

The Rust tests use those outputs through the public Common API. They cover both
overflow additions separately, high-bit folding, all byte values, every split,
little-endian typed words, flat/nested/empty arrays, and the production RNG CRC
path. Repeated seed CRC reads must leave all six words and the next draw intact.

```sh
cd GeneralsRust
cargo test --offline --locked -j 1 -p game_engine --test crc_parity
cargo test --offline --locked -j 1 -p game_engine --test crc_parity --features debug
cargo check --offline --locked -j 1 -p game_engine --lib
```

Each integration command must run nine tests. Common's unit-test crate is
suppressed unless feature `internal` is selected, so an ordinary filtered
`cargo test` reporting zero tests is not evidence. Cargo's test profile enables
overflow checks; the small actual-module reproduction additionally passes
`-C overflow-checks=yes` explicitly. Feature `debug` is a separate algorithm
branch, distinct from the test profile and feature `debug_crc`.

## Typed API boundary

The formerly unrestricted generic API could inspect padding or uninitialized
storage through safe calls. `CrcValue` deliberately narrows it to `u32` and
fixed arrays of supported values, encoded explicitly in original x86 byte
order. Unknown downstream callers are not proven compatible; other structures
must choose their fields and send explicit bytes to `compute_crc`.

From the repository root:

```sh
python3 GeneralsMD/Code/ParityHarness/tests/verify_crc_api.py \
  --output-dir "$crc_evidence/api"
```

This builds the actual CRC module as a small library, then compiles a positive
word/array control and seven negative controls. Padded structs and
`MaybeUninit<u32>` must fail all three typed helpers with the intended `E0277`
`CrcValue` diagnostic. An external trait implementation must fail on the sealed
supertrait. No negative probe is executed. `--library PATH --dependency-dir DIR`
instead checks a production Common rlib. The standalone mode does not establish
whole-crate compilation or Miri verification.

The standalone actual-module before/fixed runs keep the same fixture and assertions.
Before: normal branch 9/9 integration and 26/26 existing CRC unit tests pass;
feature `debug` fails four integration cases and two existing unit tests with
overflow panics. Its API accepts all six padded/uninitialized calls at compile
time; those calls are never executed. After: both branches pass 9/9 and 26/26,
and all seven rejection controls plus the positive control pass. Independent
carry-removal and reversed-word-byte mutations each fail the relevant fixture
assertion in both branches. A separate independent comparison also matches the
original on 393 byte/word stream cases per branch; the original fixture producer
was rerun under UBSan without diagnostics.
