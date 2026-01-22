FROM rust:1.90.0 AS builder

WORKDIR /usr/src/app
COPY . .

RUN cargo install --path ./server

FROM debian:bookworm-slim

COPY --from=builder /usr/local/cargo/bin/server /usr/local/bin/server
WORKDIR /usr/local/bin

CMD ["server"]