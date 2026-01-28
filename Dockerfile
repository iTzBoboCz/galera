FROM lukemathwalker/cargo-chef:latest AS chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/usr/local/cargo/git \
  --mount=type=cache,target=/app/target \
  cargo chef cook --release --recipe-path recipe.json

# Build application and reduce size
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/usr/local/cargo/git \
  --mount=type=cache,target=/app/target \
  cargo build --release --bin galera && strip /app/target/release/galera

# We do not need the Rust toolchain to run the binary!
FROM debian:trixie-slim AS runtime
WORKDIR app
COPY --from=builder /app/target/release/galera /usr/local/bin
RUN apt-get update \
  && apt-get install -y --no-install-recommends libmariadb3 ca-certificates \
  && rm -rf /var/lib/apt/lists/*
EXPOSE 8000
ENTRYPOINT ["/usr/local/bin/galera"]
