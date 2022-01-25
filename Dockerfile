# syntax=docker/dockerfile:1.3-labs

FROM rust:1.58 AS build

WORKDIR /opt/app

RUN <<EOF
rustup toolchain install stable-aarch64-unknown-linux-musl
rustup target add aarch64-unknown-linux-musl

apt-get install -y \
    gcc \
    musl-tools \
    wget \
    libssl-dev

wget "https://musl.cc/aarch64-linux-musl-cross.tgz"
tar -xzf aarch64-linux-musl-cross.tgz
EOF

COPY . .

ENV RUSTFLAGS="-Clink-self-contained=yes -Clinker=rust-lld -Ctarget-feature=+crt-static" \
    PATH=/opt/app/aarch64-linux-musl-cross/bin:${PATH}

RUN <<EOF
cargo build --release --target aarch64-unknown-linux-musl
EOF

FROM --platform=linux/arm64 alpine:3.14 AS app

COPY --from=build /opt/app/target/aarch64-unknown-linux-musl/release/cfdns /usr/local/bin/cfdns

ENTRYPOINT ["/usr/local/bin/cfdns"]
CMD ["help"]
