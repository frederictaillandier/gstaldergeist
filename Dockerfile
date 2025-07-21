FROM docker.io/library/rust:slim-bookworm AS builder
WORKDIR /usr/src/app
RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools musl-dev make perl && rm -rf /var/lib/apt/lists/*

# Set environment variables for cross-compilation
ENV CC_x86_64_unknown_linux_musl=musl-gcc
ENV CXX_x86_64_unknown_linux_musl=musl-g++

RUN cargo new --bin gstaldergeist
WORKDIR /usr/src/app/gstaldergeist
COPY Cargo.toml Cargo.lock ./
RUN cargo build --target x86_64-unknown-linux-musl --release && \
    rm src/*.rs && \
    rm target/x86_64-unknown-linux-musl/release/deps/gstaldergeist*
COPY src ./src
RUN cargo build --target x86_64-unknown-linux-musl --release

FROM scratch
WORKDIR /app
COPY --from=builder /usr/src/app/gstaldergeist/target/x86_64-unknown-linux-musl/release/gstaldergeist /app/gstaldergeist
CMD ["/app/gstaldergeist"]
