# Build
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY backend ./backend
COPY mcp ./mcp
COPY cli ./cli
RUN cargo build --release -p vulyk-api

# Runtime
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/vulyk-api /usr/local/bin/vulyk-api
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["vulyk-api"]
