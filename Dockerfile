FROM rust:latest as builder

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /usr/src/Datoxidize
COPY . .

# cd into client and build
RUN cd client && cargo build --release

# cd into backend and build
RUN cd backend && cargo build --release



FROM debian:buster-slim

# Create a folder for the backend to live in with all the resource files
RUN mkdir -p /usr/local/bin/backend
COPY --from=builder /usr/src/Datoxidize/target/release/backend /usr/local/bin/backend/backend

# todo - change this to add new user then chown instead as chmod 777 isn't great for security
RUN chmod -R 777 /usr/local/bin/backend

# copies the resource files from local dir to working dir
COPY ./backend/ /usr/local/bin/backend

COPY --from=builder /usr/src/Datoxidize/target/release/client /usr/local/bin/client

WORKDIR /usr/local/bin/backend
CMD ["./backend"]
LABEL service=backend

# docker built -t datoxidize .bac
# docker run -dp 8080:3000 --rm --name server1 server