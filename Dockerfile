# Build Stage
FROM rust:1.81.0-slim-bookworm AS builder

WORKDIR /usr/src/app

RUN apt-get update && \
    apt-get install -y \
        pkg-config \
        libssl-dev \
        libpq-dev \
        build-essential \
        git \
        net-tools \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

COPY . .

RUN cargo build --release

# Runtime Stage
FROM debian:bookworm-slim

ARG PROMETHEUS_PORT
ARG GRPC_ADDR
ARG LIBP2P_LISTEN_ADDRESS
ARG NODE_ADDRESS

RUN apt-get update && \
    apt-get install -y \
        libpq5 \
        ca-certificates \
        net-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

COPY --from=builder /usr/src/app/target/release/distributed-file-storage .

RUN chmod +x distributed-file-storage
RUN mkdir -p chunks

RUN useradd -m appuser
USER appuser

EXPOSE 9000 9898 50051 9001 9899 50052 9002 9900 50053

ENV RUST_LOG=info
ENV PROMETHEUS_PORT=${PROMETHEUS_PORT}
ENV GRPC_ADDR=${GRPC_ADDR}
ENV LIBP2P_LISTEN_ADDRESS=${LIBP2P_LISTEN_ADDRESS}
ENV NODE_ADDRESS=${NODE_ADDRESS}

CMD ["./distributed-file-storage"]
