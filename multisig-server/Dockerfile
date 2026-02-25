# Build stage
FROM rust:1.88-slim-bookworm AS builder

RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY multisig-server/Cargo.toml multisig-server/Cargo.lock ./
COPY multisig-server/src/ src/
COPY multisig-server/migrations/ migrations/

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/multisig-server /usr/local/bin/multisig-server

EXPOSE 8080

CMD ["multisig-server"]
