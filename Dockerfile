# --- Build stage ---
FROM rust:1.90-bookworm AS builder

ENV CARGO_BUILD_JOBS=4

# Install dx CLI and wasm target
RUN cargo install dioxus-cli@0.7.3 --locked \
    && rustup target add wasm32-unknown-unknown

# Install Node.js for Tailwind CSS
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs

WORKDIR /app

# Cache npm deps
COPY package.json package-lock.json ./
RUN npm ci

# Cache cargo deps
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo fetch

# Copy source and assets
COPY src/ src/
COPY assets/ assets/
COPY input.css Dioxus.toml ./

# Build Tailwind CSS
RUN npx @tailwindcss/cli -i ./input.css -o ./assets/main.css --minify

# Build the app
RUN dx build --release --platform web

# --- Runtime stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates tesseract-ocr && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built server binary and public assets
COPY --from=builder /app/target/dx/life_manager/release/web/life_manager ./life_manager
COPY --from=builder /app/target/dx/life_manager/release/web/public ./public

# SQLite database will be stored in a volume
VOLUME /app/data

ENV DATABASE_PATH=/app/data/life_manager.db
ENV IP=0.0.0.0
ENV PORT=8080

EXPOSE 8080

CMD ["./life_manager"]
