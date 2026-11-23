# SPDX-License-Identifier: MIT
# The Rust notification-service image (ADR 0029), built by gluon bin/k8s-local-up
# with
#   docker build -t notification-service-rs:latest \
#     --build-context gluon-rs=lib/gluon-rs services/notification-service-rs
# The shared crates are a path dependency on ../../lib/gluon-rs, so the build
# lays out both repos as on disk. Like the Scala image: port 8080 (API).

FROM rust:1.99.1-bookworm AS build
# Sources only: gluon-rs's target/ stays out.
COPY --from=gluon-rs Cargo.toml Cargo.lock /src/lib/gluon-rs/
COPY --from=gluon-rs crates /src/lib/gluon-rs/crates
WORKDIR /src/services/notification-service-rs
COPY . .
# rust-toolchain.toml pins 1.99.1, the image's own toolchain.
RUN cargo build --release --locked -p notification-service --bin notification-service

FROM debian:bookworm-slim
COPY --from=build /src/services/notification-service-rs/target/release/notification-service /usr/local/bin/notification-service
WORKDIR /app
EXPOSE 8080
# A numbered non-root user: no shell or home needed.
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/notification-service"]
