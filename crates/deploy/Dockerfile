# syntax=docker/dockerfile:1.7
# Clave node image: clave-relay and clave-keeper (for members who run their keeper on a server).
# Build from `crates/`: docker build -f deploy/Dockerfile -t clave-node .
# The `test` stage adds the network-scenario driver used by keeper/tests/network.

FROM rust:1.96-slim-bookworm AS build
WORKDIR /src
# Crate downloads stall on some networks over HTTP/2; use HTTP/1.1 with generous retries and timeouts.
ENV CARGO_HTTP_MULTIPLEXING=false CARGO_NET_RETRY=10 CARGO_HTTP_TIMEOUT=120
COPY . .
# Fetch separately so an interrupted download resumes from the cache instead of restarting the build.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo fetch --locked
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --offline -p clave-keeper -p clave-relay --bins \
 && cargo build --release --locked --offline -p clave-keeper --example network_scenario \
 && mkdir -p /out \
 && cp target/release/clave-keeper target/release/clave-relay target/release/examples/network_scenario /out/

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin clave \
 && mkdir -p /data && chown clave:clave /data && chmod 0700 /data
COPY --from=build /out/clave-keeper /out/clave-relay /usr/local/bin/
USER clave
WORKDIR /data
VOLUME /data
# libp2p (QUIC and TCP) and the relay's health/metrics endpoint.
EXPOSE 4001/udp 4001/tcp 9090/tcp
CMD ["clave-relay", "run", "--data-dir", "/data", "--health", "0.0.0.0:9090", "--log-format", "json"]

FROM runtime AS test
COPY --from=build /out/network_scenario /usr/local/bin/
