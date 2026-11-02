FROM rust:1.99-slim-trixie AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps
COPY proto ./proto
RUN cargo build --locked --release -p game-server

FROM debian:trixie-slim
RUN groupadd --gid 10001 shogi \
    && useradd --uid 10001 --gid shogi --no-create-home shogi \
    && mkdir /data \
    && chown shogi:shogi /data
COPY --from=build /app/target/release/game-server /usr/local/bin/game-server
USER shogi
ENV DATABASE_URL=sqlite:///data/shogi.db?mode=rwc
ENV GAME_SERVER_ADDR=0.0.0.0:50051
EXPOSE 50051
ENTRYPOINT ["game-server"]
