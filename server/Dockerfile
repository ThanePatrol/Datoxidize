FROM rust:1.61.0-slim-buster as builder

# Create new empty project
RUN USER=root cargo new --bin server
WORKDIR /server

# copy dependencies
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

RUN cargo build --release
RUN rm src/*.rs

# Copy source and required runtime resources
COPY ./src ./src
# static html directory
COPY ./templates ./templates
# working directory to sync files
COPY ./storage /server/storage
# .env file
COPY ./.env /server/.env
# config file
COPY ./resources /server/resources

# build for release by removing initial build then adding cargo dependencies
RUN rm ./target/release/deps/server*
RUN cargo install --path .


# startup command to run the binary
CMD ["./target/release/server"]

# https://dev.to/rogertorres/first-steps-with-docker-rust-30oi

# docker built -t server .
# docker run -dp 8080:3000 --rm --name server1 server