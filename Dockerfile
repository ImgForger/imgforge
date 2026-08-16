# Dockerfile for imgforge
#
# Debian trixie rather than Ubuntu 24.04, which ships libvips 8.15.1. The
# difference is not cosmetic: 8.15 has no HEVC encoder behind heifsave, so HEIF
# output failed at encode time with "Unsupported compression", and none of the
# encoder properties added in 8.16 — `exact`, `tune`, `keep-duplicate-frames`,
# gain-map retention for preserve_hdr — exist there at all. Trixie's 8.16.1
# encodes every format imgforge supports, AVIF and HEIF included.

# Builder stage
FROM debian:trixie-slim AS builder

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        libvips-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/imgforge

# Build the dependency graph on its own first, so that editing imgforge's own
# sources does not re-download and re-compile every crate it depends on.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && cargo build --release \
    && rm -rf src

COPY . .

# Touch the entry points so cargo rebuilds them over the placeholder build.
RUN touch src/main.rs src/lib.rs && cargo build --release

# Final stage
FROM debian:trixie-slim

# Debian ships libheif's codecs as separate plugin packages, and libvips only
# Recommends them. Installing with --no-install-recommends and stopping at
# libvips42t64 gives a build that registers an AVIF and HEIF saver and then
# fails to encode with either — which is precisely the failure imgforge now
# probes for at startup, so it degrades to a clear "unsupported format" rather
# than a 500. Naming the encoders explicitly is what makes them work at all.
#   aomenc and rav1e encode AV1 for AVIF; x265 encodes HEVC for HEIF.
#   dav1d and libde265 decode the same, for AVIF and HEIF *sources*.
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libvips42t64 \
        libheif-plugin-aomenc \
        libheif-plugin-dav1d \
        libheif-plugin-libde265 \
        libheif-plugin-x265 \
    && rm -rf /var/lib/apt/lists/*

# An image proxy fetches and decodes whatever the internet hands it, which is
# the last place to be running as root.
#
# The UID is pinned rather than left to useradd. A bind-mounted cache directory
# is owned by a numeric ID on the host, which has to be chowned to match the
# container's user before the container can write to it — and an operator can
# only do that against an ID that is stable across rebuilds.
RUN groupadd --system --gid 10001 imgforge \
    && useradd --system --uid 10001 --gid 10001 --create-home --shell /usr/sbin/nologin imgforge
WORKDIR /home/imgforge

COPY --from=builder /usr/src/imgforge/target/release/imgforge /usr/local/bin/imgforge

# The disk and hybrid caches create their storage files themselves, so the
# directory has to exist and be writable by the unprivileged user before the
# server starts — otherwise cache initialization fails and it never comes up.
# Creating them here also gives a bare `docker run -v cache:/var/cache/imgforge`
# the right ownership for free: Docker copies the image's ownership onto a fresh
# named volume, where it would otherwise be root's. Both spellings are prepared
# because both appear in the deployment documentation.
RUN mkdir -p /var/cache/imgforge /var/imgforge/cache \
    && chown -R imgforge:imgforge /var/cache/imgforge /var/imgforge

USER imgforge

# Defaults the health probe interpolates. Overriding IMGFORGE_BIND and friends
# at run time moves the probe with the server.
ENV IMGFORGE_BIND=3000 \
    IMGFORGE_PATH_PREFIX= \
    IMGFORGE_HEALTH_CHECK_PATH=/health

EXPOSE 3000

# Built from the runtime settings rather than hard-coded: a container that
# moves the listener with IMGFORGE_BIND, or the route with IMGFORGE_PATH_PREFIX
# or IMGFORGE_HEALTH_CHECK_PATH, would otherwise be probed at an address the
# server no longer answers on and reported unhealthy while working perfectly.
#
# `--noproxy '*'` because this probe is the container talking to itself. A
# deployment that sets http_proxy or ALL_PROXY for fetching sources would
# otherwise have curl send the loopback check through that proxy, which
# generally cannot reach back into the container — reporting a working server as
# unhealthy for a setting that has nothing to do with it.
#
# The listener's address is derived from IMGFORGE_BIND rather than assumed:
# a container bound to a specific interface answers only there, so probing
# 127.0.0.1 reported a perfectly healthy server as unhealthy. A bare port and
# the wildcard addresses map to loopback, which is what the server is actually
# reachable on from inside its own namespace, and an IPv6 literal is bracketed
# for the URL.
#
# The two path variables are normalized exactly the way the server normalizes
# them, and so is the bind address: `normalize_bind_address` trims its value, so
# a `IMGFORGE_BIND=' 3000 '` the server accepts must not become a URL with
# spaces in the port. `normalize_path_prefix` is `trim().trim_matches('/')` followed by one
# leading slash on a non-empty value, and `trim_matches` removes *every*
# surrounding slash rather than one — so the loops here strip repeats and the
# `tr` strips whitespace, or a value the server happily accepts as `/imgforge`
# would be probed at a different route.
#
# Interpolating them raw turned IMGFORGE_PATH_PREFIX=imgforge/ into a probe of
# `127.0.0.1:3000imgforge//health` against a live route of `/imgforge/health`,
# so the container reported itself unhealthy while serving perfectly.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD bind=$(printf %s "${IMGFORGE_BIND:-3000}" | tr -d "[:space:]"); \
        bind="${bind:-3000}"; \
        port="${bind##*:}"; \
        case "$bind" in \
            *:*) host="${bind%:*}" ;; \
            *) host="" ;; \
        esac; \
        case "$host" in \
            ""|0.0.0.0|"[::]"|::|"*") host="127.0.0.1" ;; \
            "["*"]") : ;; \
            *:*) host="[$host]" ;; \
        esac; \
        prefix=$(printf %s "${IMGFORGE_PATH_PREFIX}" | tr -d "[:space:]"); \
        while [ "${prefix#/}" != "$prefix" ]; do prefix="${prefix#/}"; done; \
        while [ "${prefix%/}" != "$prefix" ]; do prefix="${prefix%/}"; done; \
        health=$(printf %s "${IMGFORGE_HEALTH_CHECK_PATH}" | tr -d "[:space:]"); \
        while [ "${health#/}" != "$health" ]; do health="${health#/}"; done; \
        while [ "${health%/}" != "$health" ]; do health="${health%/}"; done; \
        health="${health:-health}"; \
        curl --fail --silent --noproxy '*' \
        "http://${host}:${port}${prefix:+/$prefix}/${health}" || exit 1

CMD ["imgforge"]
