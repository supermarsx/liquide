FROM rust:1.85 AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin liquid-desktopd

FROM debian:bookworm-slim
COPY --from=builder /build/target/release/liquid-desktopd /usr/bin/
EXPOSE 3389
ENTRYPOINT ["liquid-desktopd"]
