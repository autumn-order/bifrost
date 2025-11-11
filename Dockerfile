# === Generate Tailwindcss ===
FROM oven/bun:1.3-alpine AS bun_stage
WORKDIR /app

# Build Dependencies
COPY package.json bun.lock ./

RUN bun i

# Generate CSS
COPY tailwind.config.ts tailwind.css ./
COPY src ./src

RUN bunx @tailwindcss/cli -i ./tailwind.css -o ./assets/tailwind.css --minify

# === Compile Rust App ===
# Use debian bookworm-slim as DX CLI is not compiled for alpine
FROM rust:1.90-slim AS rust_stage
WORKDIR /app

# Build Dependencies
# - `pkg-config` required for `cargo build`
# - `libssl-dev` required for `cargo build`
# - `curl` required for `utoipa` crate for swagger API docs
# - `unzip` required for installing dioxus CLI
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install wasm32-unknown-unknown
RUN rustup default stable \
    && rustup target add wasm32-unknown-unknown

# Install Dioxus CLI
RUN mkdir -p /root/.cargo/bin \
    && curl -sSL https://dioxus.dev/install.sh | bash

ENV PATH="/root/.cargo/bin:${PATH}"

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./
COPY .cargo ./.cargo
COPY entity ./entity
COPY migration ./migration
COPY bifrost-test-utils ./bifrost-test-utils

# Build Rust application
COPY assets ./assets
COPY src ./src
COPY --from=bun_stage /app/assets/tailwind.css /app/assets/tailwind.css

# Cache compiled depdencies for future builds
RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    dx build --release \
    && mkdir -p /app/output \
    && cp -r /app/target/dx/bifrost/release/web /app/output/

# === Run application ===
FROM debian:bookworm-slim
ARG APP_NAME=bifrost
ARG IP=0.0.0.0
ARG PORT=8080

ENV APP_NAME=${APP_NAME}
ENV IP=${IP}
ENV PORT=${PORT}

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 ${APP_NAME}

COPY --from=rust_stage --chown=${APP_NAME}:${APP_NAME} \
    /app/output/web/ /app

USER ${APP_NAME}
EXPOSE ${PORT}

CMD ["sh", "-c", "./${APP_NAME}"]
