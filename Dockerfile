# Stage 1: Build the binary
FROM rust:latest as builder
WORKDIR /app

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY  ./manga-tui-config.toml ./manga-tui-config.toml
COPY ./src ./src
# Install required native libraries
RUN apt-get update && apt-get install -y \
        libdbus-1-dev pkg-config \
        openssl \ 
        libssl-dev \
        pkg-config \
        && rm -rf /var/lib/apt/lists/*

RUN cargo build --release

# Stage 2: Create a minimal runtime image
FROM debian:latest
WORKDIR /app
COPY --from=builder /app/target/release/manga-tui .

COPY  ./manga-tui-config.toml ./manga-tui-config.toml
COPY ./src ./src
# Install required native libraries
RUN apt-get update && apt-get install -y \
        libdbus-1-dev pkg-config \
        openssl \ 
        libssl-dev \
        pkg-config \
        && rm -rf /var/lib/apt/lists/*


# Set the terminal environment
ENV TERM=xterm-256color


# Command to run the application
CMD ["./manga-tui"]

