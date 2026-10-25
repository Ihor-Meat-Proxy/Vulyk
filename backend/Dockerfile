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
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /var/lib/vulyk --create-home vulyk
WORKDIR /app
COPY --from=builder /app/target/release/vulyk-api /usr/local/bin/vulyk-api
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
USER vulyk
HEALTHCHECK --interval=10s --timeout=5s --start-period=20s --retries=5 \
  CMD curl -fsS http://127.0.0.1:8080/health || exit 1
CMD ["vulyk-api"]
