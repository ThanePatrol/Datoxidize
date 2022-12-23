FROM rust:1.61.0-slim as builder

# Create new empty project
RUN USER=root cargo new --bin datoxidize
WORKDIR /datoxidize

# copy dependencies
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

# Build dependencies only
RUN cargo build --release
RUN rm src/*.rs

# Copy source and required runtime resources
COPY ./src ./src
# static html files
COPY ./templates ./templates
# working directory to sync files
COPY ./copy_dir ./copy_dir
# .env file
COPY ./.env ./.env
# config file
COPY ./test_resources ./test_resources

# build for release by removing initial build then adding cargo dependencies
RUN rm ./target/release/deps/datoxidize*
RUN cargo build --release

# final base
FROM rust:1.61.0

# copy build artifact from build stage
COPY --from=builder /datoxidize/target/release/datoxidize .

# startup command to run the binary
CMD ["./datoxidize"]

# https://dev.to/rogertorres/first-steps-with-docker-rust-30oi