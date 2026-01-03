# syntax=docker/dockerfile:1

FROM rust:1.85-bookworm AS builder
WORKDIR /work

# Pre-copy manifests for better layer cache
COPY Cargo.toml Cargo.lock ./
COPY cli/Cargo.toml cli/Cargo.toml
COPY core/Cargo.toml core/Cargo.toml
COPY gen/Cargo.toml gen/Cargo.toml
COPY utils/Cargo.toml utils/Cargo.toml
COPY spec/Cargo.toml spec/Cargo.toml
COPY parse/Cargo.toml parse/Cargo.toml
COPY semantic/Cargo.toml semantic/Cargo.toml

# Copy sources
COPY cli cli
COPY core core
COPY gen gen
COPY utils utils
COPY spec spec
COPY parse parse
COPY semantic semantic

RUN cargo build -p cli --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /work/target/release/cli /usr/local/bin/rsctl
ENTRYPOINT ["/usr/local/bin/rsctl"]


