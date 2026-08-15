# Roadmap

Candidate work, mostly sourced by comparing imgforge against [imgproxy](https://github.com/imgproxy/imgproxy) — its
[processing options](https://docs.imgproxy.net/usage/processing) and its pipeline source. Nothing here is committed;
it is a list of what is missing, what it would cost, and what is known about each.

Measurements below were taken on libvips 8.15.1 (the version the published image ships) unless noted.

## Where imgforge already stands

imgproxy gates a number of options behind its Pro tier that imgforge implements for free:
`resizing_algorithm`, `background_alpha`, `watermark_url`, all four `*_options` encoder groups, and the
`adjust`/`brightness`/`contrast`/`saturation` family. Compatibility work is about closing specific gaps, not
catching up wholesale.

## Performance

### Scale-on-load

**The largest single win available.** imgproxy decodes the source at a reduced size before anything else
(`processing/scale_on_load.go`): it computes `preshrink = min(wshrink, hshrink)`, normalises to the 1/2/4/8 that
JPEG supports, and reloads at that scale. imgforge always decodes at full resolution and downsamples afterwards.

Measured on a 9000×7000 JPEG:

| | Peak RSS |
| --- | --- |
| Decode full, then resize (imgforge today) | 84 MB |
| Shrink-on-load | 53 MB |

Memory is the binding constraint for this service — [Performance Tips](doc/9_performance.md) tells operators to size
`IMGFORGE_WORKERS` from measured peak memory — so ~40% off per-request peak converts fairly directly into
concurrency at the same memory ceiling. Wall-clock gains are real but smaller and content-dependent.

Not a small change: the decode currently happens in `service.rs` before the processing plan is known, and
scale-on-load needs the target scale first. Expect to restructure `process_path`. Applies to JPEG and WebP, plus
formats with embedded thumbnails.

### Colour management

imgproxy runs `colorspaceToProcessing` before scaling and `colorspaceToResult` at the end. imgforge does neither —
it processes in whatever colourspace the source arrived in. Worth investigating for colour accuracy on images with
unusual ICC profiles; no measurement yet, so the size of the problem is unknown.

## Compatibility: standard imgproxy options imgforge lacks

Every option below is free-tier in imgproxy. Ordered by usefulness relative to effort.

### `trim` / `t`

Trim uniform borders. `trim:threshold:color:equal_hor:equal_ver`.

Feasible: `ops::find_trim_with_opts` plus `extract_area`. The `line-art` property those bindings pass **does** exist
in libvips 8.15, so this does not repeat the encoder-property problem that broke AVIF and GIF. Roughly 80–120 lines
with tests.

Caveat worth documenting if implemented: imgproxy notes trim forces the image fully into memory and disables
scale-on-load, so it is expensive on large sources.

### `enforce_thumbnail` / `eth`

Use the source's embedded EXIF thumbnail when one exists and is large enough. Cheap at request time, moderate to
implement.

### `keep_copyright` / `kcr`

Retain the copyright tag when stripping metadata. Looks trivial and is not: libvips' `keep` flags are
`none|exif|xmp|iptc|icc|other|gainmap|all`, with no copyright granularity. Requires reading the EXIF copyright field
and re-attaching it after the strip.

### `preserve_hdr` / `ph`

**Blocked** on libvips ≥ 8.16. See the libvips upgrade below.

### `max_animation_frames` / `maf`, `max_animation_frame_resolution` / `mafr`

**Blocked** on animation support, which imgforge does not have — these limits have nothing to bound today.

## Known gaps in what imgforge already accepts

Options that parse successfully but do not fully apply. Each is documented in
[Processing Options](doc/5_processing_options.md); this is the summary.

- **`brightness`, `contrast`** — parsed and validated, never applied. The libvips Rust crate does not publicly expose
  the `linear` operation they need. Unblocking means either a crate contribution or dropping to the raw bindings.
- **`page`, `pages`, `disable_animation`, `skip_processing`** — accepted for URL compatibility only. Real support
  needs a multi-page decode path; today everything goes through a single-image decode.
- **`webp_options` preset** — only libvips' own preset names reach the encoder. Others are ignored rather than
  failing, because an unknown name makes libvips reject the whole encode.
- **HEIF output** — fails on a libvips built without an HEVC encoder, reporting `Unsupported compression`. AVIF uses
  a different codec and is unaffected. Consider reporting HEIF as unsupported at the format-probe stage instead of
  failing at encode time.

## Infrastructure

### libvips ≥ 8.16 in the published image

The image is built `FROM ubuntu:24.04`, which ships libvips 8.15.1. That version is why WebP, AVIF, HEIF, and GIF
all encode through hand-built save suffixes: the libvips crate's generated bindings name encoder properties added in
8.16 (`exact`, `tune`, `keep-duplicate-frames`), and an older libvips rejects the entire call.

Shipping 8.16+ would let those four formats use the generated bindings and delete the suffix builders, and unblocks
`preserve_hdr`. Noble has no such package, so it means a source build or a third-party repository — weigh that
against the maintenance the suffixes currently cost, which is low.

Upgrading the **crate** does not help: 2.3.0 passes the same property names. Verified by running both crate versions
against 8.15.1.

### Test coverage gaps

- **TIFF encoding has no test.** Its binding properties check out against 8.15 by inspection, so it is not broken,
  but it is in the position AVIF and GIF were in before their failure was found: shipping and unexercised.
- **`extend_image`** carries the same `u32 → i32` cast that caused the padding overflow. It is not reachable the
  same way — an absurd resize target wraps negative at the guard in `processing/mod.rs` and extend is silently
  skipped rather than producing a wrong image — but it is a silent no-op on nonsense input and belongs with the
  resize-target handling.

## How to extend this list

The comparisons that produced it are repeatable: read the option table in imgproxy's processing docs against
`src/processing/options.rs`, and read a pipeline stage in `imgproxy/processing/*.go` against the equivalent in
`src/processing/`. The two findings with the most impact — enlargement capping and alpha premultiplication — both
came from reading their pipeline rather than their documentation.
