FROM rust:1.81-bookworm AS chef 
RUN cargo install cargo-chef 
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates musl-tools
RUN update-ca-certificates
RUN rustup target add "$(uname -m)"-unknown-linux-musl
RUN cargo install cavif --target "$(uname -m)"-unknown-linux-musl
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --bin get_metadata --bin img2epub --bin epub2img --bin server --release --target "$(uname -m)"-unknown-linux-musl
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/get_metadata -o /get_metadata
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/img2epub -o /img2epub
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/epub2img -o /epub2img
RUN strip /app/target/"$(uname -m)"-unknown-linux-musl/release/server -o /server

FROM alpine AS converter
COPY --from=builder /get_metadata /get_metadata
COPY --from=builder /img2epub /img2epub
COPY --from=builder /epub2img /epub2img
COPY --from=builder /usr/local/cargo/bin/cavif /usr/local/bin/cavif
COPY ./convert.sh /convert.sh
RUN apk add --no-cache ca-certificates curl tar zip unzip
RUN update-ca-certificates
RUN chmod +x /convert.sh
ENTRYPOINT ["/convert.sh"]

FROM scratch AS server
WORKDIR /app
COPY --from=builder /server /server
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENTRYPOINT ["/server"]
