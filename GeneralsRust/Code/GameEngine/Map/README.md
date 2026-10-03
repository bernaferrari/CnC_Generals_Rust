# SAGE map byte decoding

`generals_map` owns the chunky-map byte reader, table of contents, bounded chunk
traversal, and a decoded document. It uses only `std` and forbids unsafe code.
It performs no filesystem resolution, decompression, engine discovery, or GPU work.

The original authority is `Common/System/DataChunk.cpp`: scalar fields are little
endian; TOC names use an unsigned byte length; ASCII/UTF-16 strings use an unsigned
short length; chunks have an ID, unsigned-short version, and signed payload length.
Repeated chunks retain stream order. Exact label lookup returns the first matching
chunk. Repeated TOC IDs use the final mapping, matching C++ prepend-and-search order.

Main retains its existing file-resolution, compression, engine Dict conversion,
and authored terrain/object/script adapters. Those adapters use this byte reader.
The live map-loading operation owns one decoded document and passes it through
parsing and script initialization. No last-loaded-map global or decompression TLS
is involved. Independent loads reread their source, so replacing a file at the same
path cannot return another operation's stale cached bytes.

Chunk traversal borrows payloads from the parent buffer. Decoding rejects negative
or impossible TOC counts before allocating and rejects negative/out-of-parent
payloads. Valid-map behavior is retained; these checks define safe failure for
malformed input. Existing short trailing-header and lossy string behavior is
preserved. This extraction does not prove complete C++ malformed-file equivalence.

Higher-level map records and gameplay installation remain in their existing
modules. This crate is the shared byte-format seam, not a second world or loader.

From `GeneralsRust`:

```sh
cargo test --locked -p generals_map
cargo test --locked -p generals_main --lib game_logic::script_loader::tests -- --test-threads=1
```
