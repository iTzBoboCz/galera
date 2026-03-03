FROM lukemathwalker/cargo-chef:latest AS chef
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
FROM debian:trixie-slim AS runtime

# Set OCI labels
LABEL org.opencontainers.image.title="Galera" \
  org.opencontainers.image.description="Galera" \
  org.opencontainers.image.source="https://github.com/itzbobocz/galera" \
  org.opencontainers.image.url="https://github.com/itzbobocz/galera" \
  org.opencontainers.image.base.name="docker.io/library/debian:trixie-slim"

WORKDIR app
COPY --from=builder /app/target/release/galera /usr/local/bin
COPY .env.default .env
RUN apt update -y && apt install -y libmariadb3
EXPOSE 8000
ENTRYPOINT ["galera"]
