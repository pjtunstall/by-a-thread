FROM rust:1.90.0 as builder

WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY common ./common
COPY server ./server

RUN cargo install --path ./server

FROM debian:bookworm-slim

COPY --from=builder /usr/local/cargo/bin/server /usr/local/bin/server
WORKDIR /usr/local/bin

CMD ["server"]