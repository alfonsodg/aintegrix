FROM rust:slim AS builder

WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
COPY tests/ tests/

RUN cargo build --release --bin aintegrix

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates sqlite3 && \
    rm -rf /var/lib/apt/lists/* && \
    useradd -r -s /bin/false aintegrix && \
    mkdir -p /var/lib/aintegrix /etc/aintegrix && \
    chown aintegrix:aintegrix /var/lib/aintegrix

COPY --from=builder /app/target/release/aintegrix /usr/local/bin/aintegrix

USER aintegrix

VOLUME ["/var/lib/aintegrix", "/etc/aintegrix"]

EXPOSE 8050

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/aintegrix", "--version"]

ENTRYPOINT ["/usr/local/bin/aintegrix"]
CMD ["--config", "/etc/aintegrix/aintegrix.yaml"]
