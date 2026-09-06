FROM rust:1.89-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 horus
COPY --from=builder /app/target/release/horus-api /usr/local/bin/horus-api
USER horus
EXPOSE 8080
CMD ["horus-api"]
