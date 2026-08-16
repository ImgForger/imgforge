# Roadmap

Candidate work, mostly sourced by comparing imgforge against [imgproxy](https://github.com/imgproxy/imgproxy) — its
[processing options](https://docs.imgproxy.net/usage/processing) and its pipeline source. Nothing here is committed;
it is a list of what is missing, what it would cost, and what is known about each.

Measurements below were taken on libvips 8.16.1 (the version the published image now ships) unless noted.

## Where imgforge stands

imgproxy gates a number of options behind its Pro tier that imgforge implements for free: `resizing_algorithm`,
`background_alpha`, `watermark_url`, all four `*_options` encoder groups, the `adjust`/`brightness`/`contrast`/
`saturation` family, and `page`/`pages`/`disable_animation`.

0.18.0 closed the free-tier gap that remained. Every option in imgproxy's free tier is now implemented rather than
merely parsed, with the exceptions listed under **Known gaps** below.

## Known gaps in what imgforge accepts

- **`keep_copyright` on non-JPEG output** — libvips' `keep` flags have no copyright granularity, so imgforge reads
  the EXIF `Copyright` and `Artist` fields from the source and splices a minimal EXIF segment into the encoded
  result. That mechanism only exists for JPEG. PNG and WebP can carry EXIF too, and the same approach would work
  for them; nobody has needed it yet.
- **`preserve_hdr` on libvips below 8.16** — the `gainmap` keep flag does not exist there. imgforge checks the
  runtime version once and drops the flag rather than naming it, so the request succeeds and loses only the gain
  map; the high bit-depth half still works. The drop is logged. The published image ships 8.16.1 and is
  unaffected.
- **`webp_options` preset** — only libvips' own preset names reach the encoder. Others are ignored rather than
  failing, because an unknown name makes libvips reject the whole encode.

## imgproxy Pro options imgforge does not implement

Listed so the comparison is honest rather than because they are planned: `autoquality`, `crop_aspect_ratio`,
`objects_position` and the object-detection family, `monochrome`, `duotone`, `colorize`, `gradient`,
`unsharp_masking`, `blur_areas`, `style`, `dpi`, `color_profile`, `hashsum`, `watermark_text`/`_size`/`_rotate`/
`_shadow`, `fallback_image_url`, and the `video_thumbnail_*` family. Smart gravity (`gravity:sm`) is Pro as well;
libvips does expose `smartcrop`, so it is the one entry here that would be cheap.

## Performance

### Scale-on-load — done

Both loaders decode at a reduced scale when the plan allows, so a large source is no longer unpacked at full
resolution to produce a small result. Measured through the real load path, downscaling a 9000×7000 source to 450px:

| Source | Before | After |
| --- | --- | --- |
| JPEG | 114 MB | 49 MB |
| WebP | 737 MB | 50 MB |

WebP gains far more because libwebp decodes the whole image at once rather than streaming, which had made a large
WebP the worst case imgforge had. Its loader also takes a continuous `scale` rather than JPEG's power-of-two
`shrink`, so it lands closer to what the request needs.

Cropped requests are included. A crop names a region of the source in pixels, so the region is rewritten against
what was actually decoded. A *fractional* crop needs no rewriting at all, since it is measured against whatever was
decoded — one of the incidental wins from making crop extents fractional.

A note on estimating: this entry originally read "expect to restructure `process_path`". It was about 40 lines,
because the processing plan is already parsed before the decode and libvips' loader already takes an option string.
Check the code before trusting a cost written here.

### Animation cost

Every frame now goes through the whole pipeline separately, which is what makes rotation and padding work on an
animation at all. It also means an N-frame source costs N times a still, and the frames are materialised rather
than streamed. `IMGFORGE_MAX_ANIMATION_FRAMES` and `IMGFORGE_MAX_ANIMATION_FRAME_RESOLUTION` exist because of this;
they are unset by default, which is the wrong default for a public deployment and the right one for an upgrade.

No measurements yet on where the practical ceiling sits. The obvious optimisation — recognising that a request
which only resizes could go through `vips_thumbnail`, which handles the stack natively — has not been attempted.

### Colour management cost

Colour conversion runs once per image rather than once per frame, and is a no-op for a source already in sRGB,
which is nearly all of them. No measurable cost has been observed on the common path, but nothing has been
measured carefully either.

## Infrastructure

### libvips 8.16 in the published image — done

The image is built `FROM debian:trixie-slim`, which ships libvips 8.16.1. Ubuntu 24.04 shipped 8.15.1, which is
why the encoder suffixes exist and why HEIF output failed at encode time there.

Debian packages libheif's codecs as separate plugins that libvips only *Recommends*, so a
`--no-install-recommends` image registers AVIF and HEIF savers that cannot encode anything. `libheif-plugin-aomenc`
and `libheif-plugin-x265` are named explicitly for that reason, with `dav1d` and `libde265` for decoding AVIF and
HEIF *sources*. Verified by encoding all seven output formats through a running container.

The build context needed a `.dockerignore`: `COPY . .` was shipping the local `target/` directory into the image,
which was slow and, once the directory held a couple of gigabytes of debug artefacts, ran the builder out of disk.

The suffix builders were kept rather than replaced with the generated bindings: they work across libvips versions,
and they are the only form that can express a *combination* of metadata `keep` flags, which `strip_metadata` and
`strip_color_profile` need in order to be independent of each other.

### Test coverage gaps

- **Animation** is covered for split, join, resize, rotation, and the frame limit, all against a real animated GIF.
  Not covered: animated WebP and AVIF output, or a source whose frames have differing sizes.
- **Colour management** has no test against a real CMYK or wide-gamut ICC source, because there is no such fixture
  in the repository and generating a correct one by hand is not obviously cheaper than checking in a file.
- **`enforce_thumbnail`** is covered for the "no thumbnail" and "malformed thumbnail" paths but not for a JPEG that
  actually carries one, for the same reason.

## How to extend this list

The comparisons that produced it are repeatable: read the option table in imgproxy's processing docs against
`src/processing/options/names.rs`, and read a pipeline stage in `imgproxy/processing/*.go` against the equivalent
in `src/processing/`. The two findings with the most impact — enlargement capping and alpha premultiplication —
both came from reading their pipeline rather than their documentation.
