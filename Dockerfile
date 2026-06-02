FROM rust:1.85-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev libmariadb-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Pre-build dependencies (layer caching)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release || true
RUN rm -rf src

# Build actual source
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 libmariadb3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/todo-app /app/todo-app

EXPOSE 8080

CMD ["/app/todo-app"]
