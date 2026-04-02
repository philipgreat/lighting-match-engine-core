FROM rust:1.87-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends clang libclang-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src

RUN cargo build --release --features redis-module-host

FROM redis:8.0-bookworm

COPY --from=builder /app/target/release/liblighting_match_engine_core.so /usr/lib/redis/modules/liblighting_match_engine_core.so

EXPOSE 6379

CMD ["redis-server", "--loadmodule", "/usr/lib/redis/modules/liblighting_match_engine_core.so"]
