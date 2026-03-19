FROM rust:slim AS builder
RUN apt-get update \
    && apt-get install -y pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# compile dependencies for caching 
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
# compile code
COPY src ./src
RUN cargo build --release

FROM ubuntu:latest
RUN apt-get update \
    && apt-get install -y \
    ca-certificates \
    && update-ca-certificates --fresh \
    && rm -rf /var/lib/apt/lists/*
USER 1000:1000
WORKDIR /app
COPY --from=builder \
    /app/target/release/deepseek-agent \
    /user/local/bin/deepseek-agent
CMD ["/user/local/bin/deepseek-agent"]