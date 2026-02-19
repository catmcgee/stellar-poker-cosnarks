FROM rust:1.79-slim AS builder
WORKDIR /app
COPY services/node/ ./services/node/
COPY Cargo.toml ./Cargo.toml
RUN cargo build --release -p mpc-node 2>/dev/null || true

# Build co-noir from source
FROM rust:1.79-slim AS conoir-builder
RUN apt-get update && apt-get install -y git pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install --git https://github.com/TaceoLabs/co-snarks --branch main co-noir

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/mpc-node /usr/local/bin/mpc-node
COPY --from=conoir-builder /usr/local/cargo/bin/co-noir /usr/local/bin/co-noir

# Party configs are mounted via docker-compose volumes
# Circuit artifacts and CRS are mounted via shared volumes

CMD ["mpc-node"]
