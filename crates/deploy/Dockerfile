# syntax=docker/dockerfile:1.7
# Tecpan node image: tecpan-relay and tecpan-keeper (for members who run their keeper on a server).
# Build from `crates/`: docker build -f deploy/Dockerfile -t tecpan-node .
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
    cargo build --release --locked --offline -p tecpan-keeper -p tecpan-relay --bins \
 && cargo build --release --locked --offline -p tecpan-keeper --example network_scenario \
 && mkdir -p /out \
 && cp target/release/tecpan-keeper target/release/tecpan-relay target/release/examples/network_scenario /out/

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin tecpan \
 && mkdir -p /data && chown tecpan:tecpan /data && chmod 0700 /data
COPY --from=build /out/tecpan-keeper /out/tecpan-relay /usr/local/bin/
USER tecpan
WORKDIR /data
VOLUME /data
# libp2p (QUIC and TCP) and the relay's health/metrics endpoint.
EXPOSE 4001/udp 4001/tcp 9090/tcp
CMD ["tecpan-relay", "run", "--data-dir", "/data", "--health", "0.0.0.0:9090", "--log-format", "json"]

FROM runtime AS test
COPY --from=build /out/network_scenario /usr/local/bin/
