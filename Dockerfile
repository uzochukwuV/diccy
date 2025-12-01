FROM rust:1.86-slim

SHELL ["bash", "-c"]

RUN apt-get update && apt-get install -y \
    pkg-config \
    protobuf-compiler \
    clang \
    make \
    curl

RUN cargo install --locked linera-service@0.15.6 linera-storage-service@0.15.6

WORKDIR /build

HEALTHCHECK CMD ["curl", "-s", "http://localhost:5173"]

ENTRYPOINT bash /build/run.bash