# syntax=docker/dockerfile:1

# Versions come from .mise/config.toml; CI passes them as build arguments.
ARG RUST_VERSION=1.100.0
ARG NODE_VERSION=26.13.1

# Every build stage runs on the build host and produces output for the target
# platform, so multi-arch builds need no emulation. Stages built for the
# target platform contain no RUN steps for the same reason.

# ==============================================================================
# Backend: cross-compiled to a static musl binary with cargo-zigbuild
# ==============================================================================
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-alpine AS rust-builder
ARG TARGETARCH

RUN apk add --no-cache musl-dev zig \
  && cargo install --locked cargo-zigbuild@0.24.1 \
  && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

WORKDIR /app

# Build the dependencies first so they are cached between source changes.
COPY ./backend/Cargo.toml ./backend/Cargo.lock ./
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-musl ;; \
      arm64) echo aarch64-unknown-linux-musl ;; \
      *) echo "unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac > /target \
  && mkdir src && echo "fn main() {}" > src/main.rs && touch src/lib.rs \
  && cargo zigbuild --locked --release --target "$(cat /target)" \
  && rm -rf src

COPY ./backend/src ./src
RUN touch src/main.rs src/lib.rs \
  && cargo zigbuild --locked --release --target "$(cat /target)" \
  && cp "target/$(cat /target)/release/backend" /backend

# ==============================================================================
# Frontend: built on the build host, with the target's native modules
# ==============================================================================
FROM --platform=$BUILDPLATFORM node:${NODE_VERSION}-alpine AS node-builder
ARG TARGETARCH

WORKDIR /app

COPY ./frontend/package.json ./frontend/package-lock.json ./
RUN npm ci --no-audit --no-fund

COPY ./frontend ./
ENV NEXT_TELEMETRY_DISABLED=1
RUN npm run build

# The standalone output carries sharp's native binaries for the build host.
# Replace them with the target platform's.
RUN case "$TARGETARCH" in \
      amd64) cpu=x64 ;; \
      arm64) cpu=arm64 ;; \
      *) echo "unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
  && version="$(node -p "require('./node_modules/sharp/package.json').version")" \
  && mkdir /tmp/sharp && cd /tmp/sharp \
  && npm init -y >/dev/null \
  && npm install --no-audit --no-fund --os=linux --libc=musl --cpu="$cpu" "sharp@$version" \
  && rm -rf /app/.next/standalone/node_modules/@img \
  && cp -r node_modules/@img /app/.next/standalone/node_modules/@img

# ==============================================================================
# Runtime
# ==============================================================================
FROM node:${NODE_VERSION}-alpine AS runtime

WORKDIR /app

COPY --chmod=755 ./scripts/entrypoint.sh ./scripts/run_wrapper.sh ./
COPY --chmod=755 ./scripts/healthcheck.sh /usr/local/bin/healthcheck.sh

COPY --from=rust-builder --chown=node:node --chmod=755 /backend ./backend

COPY --from=node-builder --chown=node:node /app/public ./frontend/public
COPY --from=node-builder --chown=node:node /app/.next/standalone ./frontend
COPY --from=node-builder --chown=node:node /app/.next/static ./frontend/.next/static

USER node

ENV FRONTEND_BIND_HOST="0.0.0.0"
ENV FRONTEND_BIND_PORT="3000"
ENV FRONTEND_TO_BACKEND_URL="http://127.0.0.1"
ENV BACKEND_BIND_HOST="127.0.0.1"
ENV BACKEND_BIND_PORT="8080"

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
  CMD ["/usr/local/bin/healthcheck.sh"]

ENTRYPOINT ["./entrypoint.sh"]
CMD ["./run_wrapper.sh"]
