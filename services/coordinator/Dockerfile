FROM rust:1.79-slim AS builder
WORKDIR /app
COPY services/coordinator/ ./services/coordinator/
COPY Cargo.toml ./Cargo.toml
RUN cargo build --release -p coordinator 2>/dev/null || true

# Build co-noir from source
FROM rust:1.79-slim AS conoir-builder
RUN apt-get update && apt-get install -y git pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install --git https://github.com/TaceoLabs/co-snarks --branch main co-noir

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/coordinator /usr/local/bin/coordinator
COPY --from=conoir-builder /usr/local/cargo/bin/co-noir /usr/local/bin/co-noir

# Pre-compiled circuit artifacts (JSON from nargo compile)
COPY circuits/deal_valid/target/ /app/circuits/deal_valid/target/
COPY circuits/reveal_board_valid/target/ /app/circuits/reveal_board_valid/target/
COPY circuits/showdown_valid/target/ /app/circuits/showdown_valid/target/

CMD ["coordinator"]
