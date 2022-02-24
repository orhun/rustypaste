FROM ekidd/rust-musl-builder:latest as builder
WORKDIR /home/rust/src
COPY Cargo.toml Cargo.toml
RUN mkdir -p src/
RUN echo "fn main() {println!(\"failed to build\")}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/rustypaste*
COPY . .
RUN cargo build --locked --release
RUN mkdir -p build-out/
RUN cp target/x86_64-unknown-linux-musl/release/rustypaste build-out/

FROM scratch
WORKDIR /app
COPY --from=builder \
    /home/rust/src/build-out/rustypaste \
    /home/rust/src/config.toml ./
ENV SERVER__ADDRESS=0.0.0.0:8000
EXPOSE 8000
USER 1000:1000
CMD ["./rustypaste"]
