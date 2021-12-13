FROM debian:buster-slim

RUN apt update -y && apt install -y libmariadb-dev

COPY target/release/galera /usr/local/bin

RUN strip /usr/local/bin/galera

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c", "ROCKET_DATABASES={galera={url=${DATABASE_URL}}} ROCKET_ADDRESS=0.0.0.0 RUST_LOG=${LOG_LEVEL} RUST_BACKTRACE=${RUST_BACKTRACE} /usr/local/bin/galera"]
