# ========================
# Build Stage
# ========================
FROM rust:1.86.0-alpine3.20 AS builder

# Build platform argument (x86_64 or aarch64) (default: x86_64)
ARG TARGETARCH=x86_64
RUN echo "TARGETARCH: $TARGETARCH"

# Install build dependencies, including static OpenSSL libraries
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    build-base \
    curl

# Install cross-compiler toolchain only for ARM (Apple Silicon)
RUN if [ "$TARGETARCH" = "aarch64" ]; then \
        wget -qO- https://musl.cc/aarch64-linux-musl-cross.tgz | tar -xz -C /usr/local && \
        echo "/usr/local/aarch64-linux-musl-cross/bin" > /tmp/musl_cross_path; \
    else \
        echo "" > /tmp/musl_cross_path; \
    fi

# Set PATH only if we installed the cross compiler (will be empty string for x86)
ENV PATH="$(cat /tmp/musl_cross_path):$PATH"

# Set environment variables for static linking with OpenSSL
ENV OPENSSL_STATIC=yes
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include

# Add the MUSL target for static linking
RUN rustup target add $TARGETARCH-unknown-linux-musl

# Set the working directory
WORKDIR /usr/src/app

# Copy over Cargo.toml and Cargo.lock for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy over all the source code
COPY . .

# Add build argument for binary selection (homeserver or testnet)
ARG BUILD_TARGET=testnet

# Build the project in release mode for the MUSL target
RUN cargo build --release --bin pubky-$BUILD_TARGET --target $TARGETARCH-unknown-linux-musl

# Strip the binary to reduce size
RUN strip target/$TARGETARCH-unknown-linux-musl/release/pubky-$BUILD_TARGET

# ========================
# Runtime Stage
# ========================
FROM alpine:3.20

ARG TARGETARCH=x86_64
ARG BUILD_TARGET=testnet

# Install runtime dependencies (only ca-certificates)
RUN apk add --no-cache ca-certificates

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/$TARGETARCH-unknown-linux-musl/release/pubky-$BUILD_TARGET /usr/local/bin/homeserver

# Set the working directory
WORKDIR /usr/local/bin

# Expose the port the homeserver listens on (should match that of config.toml)
EXPOSE 6287

# Set the default command to run the binary
CMD ["homeserver"]
