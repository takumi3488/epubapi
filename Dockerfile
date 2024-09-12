FROM rust:1.81-alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev nasm curl
RUN cargo install cavif
COPY . .
RUN cargo build --bin get_metadata --bin img2epub --bin epub2img --bin server --release
RUN strip /app/target/release/get_metadata -o /get_metadata
RUN strip /app/target/release/img2epub -o /img2epub
RUN strip /app/target/release/epub2img -o /epub2img
RUN strip /app/target/release/server -o /server

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

FROM gcr.io/distroless/cc-debian12 AS server
WORKDIR /app
COPY --from=builder /server /server
ENTRYPOINT ["/server"]
