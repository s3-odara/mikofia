# syntax=docker/dockerfile:1

FROM rust:slim-bookworm AS builder

ENV CARGO_TERM_COLOR=always \
    RUST_BACKTRACE=1

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      build-essential \
      pkg-config \
      libglib2.0-dev \
      libgtk-3-dev \
      clang \
      cmake \
      python3 \
      python3-pip \
      git \
      curl \
      ninja-build \
      libssl-dev \
      zstd \
      ca-certificates && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt

RUN cargo install sccache --locked

WORKDIR /app
