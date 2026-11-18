# Stap 1: Bouw de Rust-applicatie (Geschikt voor Docker Compose én Google Cloud Run)
FROM rust:1-bookworm AS builder
WORKDIR /usr/src/blogrust
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# Stap 2: Lichte runtime-container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libsqlite3-0 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
RUN mkdir -p /app/data
COPY --from=builder /usr/src/blogrust/target/release/blogrust /app/blogrust
COPY posts ./posts
COPY plugins ./plugins
COPY themes ./themes
COPY media ./media
COPY scaffolds ./scaffolds
EXPOSE 3000
CMD ["/app/blogrust"]
