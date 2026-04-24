FROM debian:12-slim
WORKDIR /home/rust/src/target/release
COPY ci-bin/ ./
