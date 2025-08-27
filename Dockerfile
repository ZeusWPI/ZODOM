FROM rust:1.88-trixie AS builder
LABEL authors="nathanonraet"

RUN apt-get update && apt-get install -y libssl-dev build-essential cmake ninja-build

WORKDIR /usr/src/zodom

COPY Cargo.toml Cargo.lock ./

# cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src .cargo/

COPY ./templates ./templates
COPY ./src ./src

# make cargo detect new files
RUN touch ./src/main.rs
RUN cargo build --release


FROM debian:trixie-slim

RUN apt-get update && apt-get install -y libssl-dev

COPY ./static ./static

COPY --from=builder /usr/src/zodom/target/release/zodom /usr/local/bin/

CMD ["/usr/local/bin/zodom"]