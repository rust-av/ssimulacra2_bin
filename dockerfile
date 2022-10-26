# Build project
FROM rust:latest as builder
WORKDIR /volume
COPY . .
RUN apt-get update && apt-get install -y libavutil-dev libavformat-dev clang && rm -rf /var/lib/apt/lists/*
RUN env RUSTFLAGS="-C target-cpu=haswell" cargo build --profile=release

# Setup actual executing image
FROM debian
RUN apt-get update && apt-get install -y ffmpeg && rm -rf /var/lib/apt/lists/*
COPY --from=builder /volume/target/release/ssimulacra2_rs /ssimulacra2_rs
RUN mkdir /files

ENTRYPOINT ["/ssimulacra2_rs"]
