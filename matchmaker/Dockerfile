FROM rust:1.90.0 AS builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

COPY common ./common
COPY matchmaker ./matchmaker

# Create a dummy client to satisfy `cargo install`.
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs

# Create a dummy server to satisfy `cargo install`.
RUN mkdir -p server/src && \
    echo '[package]\nname = "server"\nversion = "0.0.0"\n[dependencies]' > server/Cargo.toml && \
    echo 'fn main() {}' > server/src/main.rs

RUN cargo install --path ./matchmaker

FROM debian:bookworm-slim

RUN rm -rf /var/lib/apt/lists/* \
    && rm -rf /usr/bin/apt* /usr/bin/dpkg /usr/bin/dash /usr/bin/bash \
    && useradd -r -s /bin/false matchmakeruser

COPY --from=builder /usr/local/cargo/bin/matchmaker /usr/local/bin/matchmaker

USER matchmakeruser

WORKDIR /usr/local/bin

CMD ["matchmaker"]
