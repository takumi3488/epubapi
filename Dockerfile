FROM rust:1.76-bookworm AS chef 
RUN cargo install cargo-chef 
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates musl-tools
RUN update-ca-certificates
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN rustup target add "$(uname -m)"-unknown-linux-musl
RUN cargo build --bin get_metadata --bin img2epub --bin server --release --target "$(uname -m)"-unknown-linux-musl
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/get_metadata -o /get_metadata
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/img2epub -o /img2epub
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/server -o /server

FROM scratch AS get_metadata
COPY --from=builder /get_metadata /get_metadata
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENTRYPOINT ["/get_metadata"]

FROM alpine AS img2epub
COPY --from=builder /img2epub /img2epub
RUN apk add --no-cache ca-certificates tar zip
RUN update-ca-certificates
ENTRYPOINT ["/img2epub"]

FROM scratch AS server
COPY --from=builder /server /server
ENTRYPOINT ["/server"]
