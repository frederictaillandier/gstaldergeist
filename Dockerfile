FROM docker.io/library/rust:slim-bookworm as builder

WORKDIR /usr/src/app
COPY . .

# Build the application
RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/gstaldergeist /app/gstaldergeist

# Create a script to set environment variables and run the app
COPY --from=builder /usr/src/app/start.sh /app/start.sh
RUN chmod +x /app/start.sh

CMD ["/app/start.sh"]
