FROM rust:1.99-slim-trixie AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps
COPY proto ./proto
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --locked --release -p game-server -p outbox-publisher -p user-server -p player-server \
    && cp target/release/game-server target/release/outbox-publisher target/release/user-server target/release/player-server /usr/local/bin/

FROM debian:trixie-slim
RUN groupadd --gid 10001 shogi \
    && useradd --uid 10001 --gid shogi --no-create-home shogi \
    && mkdir /data \
    && chown shogi:shogi /data
COPY --from=build /usr/local/bin/game-server /usr/local/bin/game-server
COPY --from=build /usr/local/bin/outbox-publisher /usr/local/bin/outbox-publisher
COPY --from=build /usr/local/bin/user-server /usr/local/bin/user-server
COPY --from=build /usr/local/bin/player-server /usr/local/bin/player-server
USER shogi
ENV DATABASE_URL=sqlite:///data/shogi.db?mode=rwc
ENV GAME_SERVER_ADDR=0.0.0.0:50051
EXPOSE 50051
ENTRYPOINT ["game-server"]
