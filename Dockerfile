FROM rust:1.72 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
WORKDIR /app
COPY --from=builder /app/target/release/distributed-file-storage /usr/local/bin/distributed-file-storage
COPY .env /app/.env
CMD ["distributed-file-storage"]
