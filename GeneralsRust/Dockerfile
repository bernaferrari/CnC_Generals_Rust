# Multi-stage build for GeneralsRust
FROM rust:1.75 as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libx11-dev \
    libxi-dev \
    libxrandr-dev \
    libxcursor-dev \
    libxxf86vm-dev \
    libasound2-dev \
    libpulse-dev \
    libudev-dev \
    libvulkan-dev \
    libgl1-mesa-dev \
    libegl1-mesa-dev \
    libwayland-dev \
    libxkbcommon-dev \
    clang \
    llvm \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /usr/src/generals-rust

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all crate directories
COPY Code ./Code
COPY tools ./tools

# Build release binary
RUN cargo build --release --workspace

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libx11-6 \
    libxi6 \
    libxrandr2 \
    libxcursor1 \
    libxxf86vm1 \
    libasound2 \
    libpulse0 \
    libudev1 \
    libvulkan1 \
    libgl1 \
    libegl1 \
    libwayland-client0 \
    libxkbcommon0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy built binaries from builder
COPY --from=builder /usr/src/generals-rust/target/release/generals /app/

# Copy assets if they exist
COPY --from=builder /usr/src/generals-rust/Code/Main/assets /app/assets 2>/dev/null || true

# Set user
RUN useradd -m -u 1000 generals && chown -R generals:generals /app
USER generals

# Expose any ports if needed
# EXPOSE 8080

# Run the game
CMD ["./generals"]
