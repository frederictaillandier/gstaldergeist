FROM docker.io/library/rust:slim-bookworm as builder
WORKDIR /usr/src/app
RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*
RUN cargo new --bin gstaldergeist
WORKDIR /usr/src/app/gstaldergeist
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release && \
    rm src/*.rs && \
    rm target/release/deps/gstaldergeist*
COPY src ./src
RUN cargo build --release

FROM docker.io/library/debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y libssl3 ca-certificates
COPY --from=builder /usr/src/app/gstaldergeist/target/release/gstaldergeist /app/gstaldergeist
CMD ["/app/gstaldergeist"]
