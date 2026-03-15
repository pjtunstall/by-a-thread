FROM rust:1.90.0 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y --no-install-recommends musl-tools && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

COPY common ./common
COPY server ./server

# Create a dummy client to satisfy `cargo install` without polluting the Docker
# build context with the real client code.
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs

# Create a dummy matchmaker to satisfy `cargo install` without polluting the
# Docker build context with the real matchmaker code.
RUN mkdir -p matchmaker/src && \
    echo '[package]\nname = "matchmaker"\nversion = "0.0.0"\n[dependencies]' > matchmaker/Cargo.toml && \
    echo 'fn main() {}' > matchmaker/src/main.rs

RUN cargo install --path ./server --target x86_64-unknown-linux-musl

RUN groupadd -r serveruser && useradd -r -g serveruser -s /bin/false serveruser

FROM scratch

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
USER serveruser

COPY --from=builder /usr/local/cargo/bin/server /server

CMD ["/server"]
