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

RUN rm -rf /var/lib/apt/lists/* \
    && rm -rf /usr/bin/apt* /usr/bin/dpkg /usr/bin/dash /usr/bin/bash
    
RUN useradd -r -s /bin/false serveruser

COPY --from=builder /usr/local/cargo/bin/server /usr/local/bin/server

USER serveruser

WORKDIR /usr/local/bin

CMD ["server"]