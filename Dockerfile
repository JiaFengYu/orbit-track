FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /build/target/release/orbit-track /usr/local/bin/orbit-track
COPY fixtures/*.tle fixtures/*.meta.json fixtures/SGP4-LICENSE ./fixtures/
COPY scripts/container-entrypoint.sh /usr/local/bin/container-entrypoint
RUN chmod 755 /usr/local/bin/container-entrypoint
USER 10001:10001
ENV PORT=10000
EXPOSE 10000
ENTRYPOINT ["/usr/local/bin/container-entrypoint"]
