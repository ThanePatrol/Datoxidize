FROM rust:1.61.0 as builder
WORKDIR /usr/src
RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev

#RUN USER=root cargo new Datoxidize
WORKDIR /usr/src/Datoxidize

COPY . .
RUN cargo build --target x86_64-unknown-linux-musl --release

# startup command to run the binary
FROM scratch
COPY --from=builder /usr/src/Datoxidize/target/x86_64-unknown-linux-musl/release/backend ./
CMD ["./backend"]
LABEL service=backend

FROM scratch
COPY --from=builder /usr/src/Datoxidize/target/x86_64-unknown-linux-musl/release/client ./
CMD ["./client"]
LABEL service=client

# https://dev.to/rogertorres/first-steps-with-docker-rust-30oi

# docker built -t datoxidize .bac
# docker run -dp 8080:3000 --rm --name server1 server