FROM lukemathwalker/cargo-chef:latest-rust-1.53.0 AS chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application and reduce size
COPY . .
RUN cargo build --release --bin galera && strip /app/target/release/galera

# We do not need the Rust toolchain to run the binary!
FROM debian:buster-slim AS runtime
WORKDIR app
COPY --from=builder /app/target/release/galera /usr/local/bin
RUN apt update -y && apt install -y libmariadb-dev
EXPOSE 8000
ENTRYPOINT ["/bin/sh", "-c", "ROCKET_DATABASES={galera={url=${DATABASE_URL}}} ROCKET_ADDRESS=0.0.0.0 RUST_LOG=${LOG_LEVEL} RUST_BACKTRACE=${RUST_BACKTRACE} /usr/local/bin/galera"]
