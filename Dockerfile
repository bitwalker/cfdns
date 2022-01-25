# syntax=docker/dockerfile:1.3-labs

FROM --platform=linux/arm64 rust:1.58-alpine3.14 AS build

WORKDIR /opt/app

RUN <<EOF
apk add --no-cache --update-cache \
    libgcc \
    libc-dev \
    openssl-dev
EOF

COPY . .

RUN <<EOF
cargo build --release --target aarch64-unknown-linux-musl
EOF

FROM --platform=linux/arm64 alpine:3.14 AS app

COPY --from=build /opt/app/target/aarch64-unknown-linux-musl/release/cfdns /usr/local/bin/cfdns

ENTRYPOINT ["/usr/local/bin/cfdns"]
CMD ["help"]
