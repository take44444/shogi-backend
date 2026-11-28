FROM rust:1.100.0-slim-trixie AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY apps ./apps
COPY proto ./proto
COPY tools/traceability-attributes ./tools/traceability-attributes
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --locked --release -p user-http --example verification-user-server \
    && cp target/release/examples/verification-user-server /usr/local/bin/

FROM debian:trixie-slim
RUN groupadd --gid 10001 shogi \
    && useradd --uid 10001 --gid shogi --no-create-home shogi
COPY --from=build /usr/local/bin/verification-user-server /usr/local/bin/verification-user-server
USER shogi
ENTRYPOINT ["verification-user-server"]
