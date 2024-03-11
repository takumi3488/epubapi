FROM rust:1.74-bookworm AS builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y musl-tools
RUN rustup target add "$(uname -m)"-unknown-linux-musl
RUN cargo build --bin server --release --target "$(uname -m)"-unknown-linux-musl
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/server -o /server

FROM scratch
COPY --from=builder /server /server
ENTRYPOINT ["/server"]
