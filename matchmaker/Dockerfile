FROM rust:1.90.0 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y --no-install-recommends musl-tools && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

COPY common ./common
COPY matchmaker ./matchmaker

# Create a dummy client to satisfy `cargo install` without polluting the Docker
# build context with the real client code.
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs

# Create a dummy server to satisfy `cargo install` without polluting the Docker
# build context with the real server code.
RUN mkdir -p server/src && \
    echo '[package]\nname = "server"\nversion = "0.0.0"\n[dependencies]' > server/Cargo.toml && \
    echo 'fn main() {}' > server/src/main.rs

RUN cargo install --path ./matchmaker --target x86_64-unknown-linux-musl

RUN groupadd -r matchmakeruser && useradd -r -g matchmakeruser -s /bin/false matchmakeruser

FROM scratch

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
USER matchmakeruser

COPY --from=builder /usr/local/cargo/bin/matchmaker /matchmaker

CMD ["/matchmaker"]
