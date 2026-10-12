# syntax=docker/dockerfile:1
# wpg in a container (TODO §2.8). Multi-stage: the web UI (Trunk, WASM) and the server
# are built in a Rust image, the runtime image only holds the binary and the sample
# templates.
#
#   docker build -t wpg .
#   docker build -t wpg --build-arg FEATURES=pcap .       # with the libpcap backend
#   docker compose up -d                                  # see docker-compose.yml
#
# Sending needs the host network and CAP_NET_RAW / CAP_NET_ADMIN:
#   docker run --rm --network host --cap-drop ALL --cap-add NET_RAW --cap-add NET_ADMIN \
#     -e WPG_PASSWORD=... -v wpg-data:/var/lib/wpg wpg
# See docs/en/src/operations/docker.md.

ARG RUST_IMAGE=rust:1-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim

# ---- build ------------------------------------------------------------------
FROM ${RUST_IMAGE} AS build
ARG TRUNK_VERSION=0.21.14
# Extra cargo features of wpg-cli, e.g. "pcap".
ARG FEATURES=""

# Behind a TLS-intercepting proxy, pass its CA certificate(s) (PEM) as a build secret:
#   docker build --secret id=ca_certs,src=/path/to/proxy-ca.pem .
# They are trusted in this build stage only and never reach the runtime image.
RUN --mount=type=secret,id=ca_certs \
    if [ -s /run/secrets/ca_certs ]; then \
        cp /run/secrets/ca_certs /usr/local/share/ca-certificates/wpg-build-ca.crt \
        && update-ca-certificates; \
    fi
RUN apt-get update \
    && apt-get install -y --no-install-recommends libpcap-dev \
    && rm -rf /var/lib/apt/lists/*
RUN arch="$(uname -m)" \
    && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${arch}-unknown-linux-gnu.tar.gz" \
       | tar -xz -C /usr/local/bin trunk \
    && trunk --version

WORKDIR /src
COPY . .
# rust-toolchain.toml pins the channel and the wasm32 target; rustup installs them here.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    rustup show active-toolchain \
    && (cd crates/wpg-web && trunk build --release) \
    && cargo build --release --locked -p wpg-cli ${FEATURES:+--features "$FEATURES"} \
    && install -D -m 0755 target/release/wpg /out/wpg

# ---- runtime ----------------------------------------------------------------
FROM ${RUNTIME_IMAGE}
ARG FEATURES=""
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
       $(case ",${FEATURES}," in *,pcap,*) echo libpcap0.8 ;; esac) \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /out/wpg /usr/local/bin/wpg
COPY templates /usr/share/wpg/templates

# The server listens on all addresses inside the container, so it refuses to start
# without a password: set WPG_PASSWORD (or WPG_AUTH__PASSWORD_FILE with a secret).
ENV WPG_DATA_DIR=/var/lib/wpg \
    WPG_SERVER__BIND=0.0.0.0:8080 \
    WPG_TEMPLATES__LIBRARY_PATHS='["/usr/share/wpg/templates"]' \
    WPG_LOG__ANSI=false

# Runs as root inside the container: raw sockets need CAP_NET_RAW / CAP_NET_ADMIN,
# which the compose file grants while dropping every other capability.
VOLUME ["/var/lib/wpg"]
WORKDIR /var/lib/wpg
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD port="${WPG_SERVER__BIND##*:}"; curl -fsS "http://127.0.0.1:${port}/api/health" >/dev/null \
        || curl -fsSk "https://127.0.0.1:${port}/api/health" >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/wpg"]
CMD ["serve"]
