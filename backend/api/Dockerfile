ARG TOOLCHAIN="1.61.0"
ARG ZIG_VERSION="0.9.1"
ARG ALPINE_VERSION="3.16.0"

####################
## BASE           ##
####################

# This stage does a bit of extra work to try and improve layer caching and build performance. See individual comments for
# more details.
# 
# The base image also has cargo-zigbuild and ziglang installed for _much_ easier cross-compilation to musl.

FROM rust:$TOOLCHAIN AS base
ARG ZIG_VERSION
# Currently, it can take a substantial amount of time to run any cargo command which updates the crates-io index.
# A work-around is to use the new `sparse-registry` unstable feature flag, but that is only available with nightly rust.
# So we clone the index ourselves inside this image so that there is a cached layer holding that information, and then
# copy in our custom cargo config to tell cargo to use the local index instead.
RUN git clone https://github.com/rust-lang/crates.io-index.git /.cargo-build-index
# Create a .cargo direction at the lowest level so that we can set global cargo config
RUN mkdir /.cargo
# Switches the registry to the crates-io git repository so that we use less memory when updating the registry.
COPY ./backend/api/docker/.cargo/config.toml /.cargo/config.toml

# Install Zig
RUN cargo install cargo-zigbuild
RUN curl -L "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-$(uname -m)-${ZIG_VERSION}.tar.xz" | tar -J -x -C /usr/local
RUN ln -s "/usr/local/zig-linux-$(uname -m)-${ZIG_VERSION}/zig" /usr/local/bin/zig

# Create the project dirs so that we don't need to later
RUN mkdir -p /shared/rust/src
RUN mkdir -p /backend/core/src
RUN mkdir -p /backend/api/src

RUN rustup target add x86_64-unknown-linux-musl

####################
## DEPENDENCIES   ##
####################

# Build just the dependencies.
#
# We create dummy crates with just the Cargo.toml files and a lib.rs, and tell all the dependencies for these crates. In 
# the actual build step, we then update the timestamps on the Cargo.toml and lib.rs files so that cargo can rebuild the 
# _local_ dependencies and the app itself.
# This has a drawback that a change in backend/api will trigger a rebuild for the shared/rust crate, but
# overall, that's an easy price to pay.

FROM base AS dependencies
COPY ./shared/rust/Cargo.toml /shared/rust/Cargo.toml
RUN touch /shared/rust/src/lib.rs

COPY ./backend/core/Cargo.toml /backend/core/Cargo.toml
RUN touch /backend/core/src/lib.rs

COPY ./backend/api/Cargo.toml /backend/api/Cargo.toml
# For the main api package, we need to include the lock file so that cargo builds the correct versions of our dependencies.
COPY ./backend/api/Cargo.lock /backend/api/Cargo.lock
RUN touch /backend/api/src/lib.rs

# Build just our dependencies
WORKDIR /backend/api
RUN cargo zigbuild --release --target x86_64-unknown-linux-musl

####################
## BUILDER        ##
####################

# Build the actual project.
#
# This stage copies _all_ our project files over, and also copies the entire target directory generated in the api-cooker
# stage. `cargo build` does the rest of the work.

FROM base AS builder
COPY ./shared /shared
COPY ./backend/core /backend/core
COPY ./backend/api /backend/api
COPY --from=dependencies /backend/api/target /backend/api/target

# We need to touch these files so that their timestamps are newer than what was used in the api-cooker layer, otherwise
# cargo will not know to rebuild these crates. See the comment in the api-cooker stage.
# Very hacky indeed.
RUN touch /shared/rust/Cargo.toml
RUN touch /shared/rust/src/lib.rs
RUN touch /backend/core/Cargo.toml
RUN touch /backend/core/src/lib.rs
RUN touch /backend/api/Cargo.toml
RUN touch /backend/api/Cargo.lock
RUN touch /backend/api/src/lib.rs

# Build the actual application
WORKDIR /backend/api
RUN cargo zigbuild --release --no-default-features --target x86_64-unknown-linux-musl

####################
## App            ##
####################

# Creates a minimal image for running the app, but without setting the entrypoint or command to run.
# When running the container, the `PROJECT_ID` should be passed in as an environment variable, and the
# command should be set.
#
# Valid project ID's are:
# - ji-cloud (release)
# - ji-cloud-developer-sandbox (sandbox)
#
# Valid commands:
# - ji-cloud-api [release|sandbox]
# - media-watch [release|sandbox]


FROM alpine:$ALPINE_VERSION as app

# Used at runtime
# ENV PROJECT_ID=ji-cloud

RUN apk --no-cache add ca-certificates

RUN mkdir -p /usr/local/bin

COPY --from=builder \
    /backend/api/target/x86_64-unknown-linux-musl/release/ji-cloud-api \
    /usr/local/bin/ji-cloud-api

COPY --from=builder \
    /backend/api/target/x86_64-unknown-linux-musl/release/media-watch \
    /usr/local/bin/media-watch

WORKDIR /usr/local/bin/