FROM rust:1.90.0 AS builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

COPY common ./common
COPY server ./server

# Create a dummy client to satisfy `cargo install`.
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs

RUN cargo install --path ./server

FROM debian:bookworm-slim

COPY --from=builder /usr/local/cargo/bin/server /usr/local/bin/server
WORKDIR /usr/local/bin

CMD ["server"]