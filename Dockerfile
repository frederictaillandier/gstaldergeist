FROM docker.io/library/rust:slim-bookworm as builder
WORKDIR /usr/src/app
COPY . .
RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN cargo build --release

FROM docker.io/library/debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y libssl3 ca-certificates
COPY --from=builder /usr/src/app/target/release/gstaldergeist /app/gstaldergeist
CMD ["/app/gstaldergeist"]
