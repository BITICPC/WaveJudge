# Step 1: Build application
FROM rust:1.40-stretch AS build

WORKDIR /app
# Copy all application related files and directories into the image.
COPY ./ ./

# Use TUNA package repository.
RUN ./docker/tuna.py

# Install the build-essential meta package.
RUN apt-get update && apt-get --assume-yes install build-essential

# Build libseccomp and openssl from source.
WORKDIR /deps
RUN git clone https://github.com/seccomp/libseccomp.git
WORKDIR /deps/libseccomp
RUN ./autogen.sh && ./configure && make && make install

WORKDIR /deps
RUN git clone https://github.com/openssl/openssl.git
WORKDIR /deps/openssl
RUN ./config && make && make install

# And then build WaveJudge itself.
WORKDIR /app
ARG profile=release
RUN ./build.py --profile $profile

# Step 2: Build application runtime based on a fresh debian image
FROM debian:stretch-slim AS runtime
WORKDIR /deps

# Install python3.
RUN apt-get --assume-yes update && apt-get --assume-yes install python3

COPY docker/ ./scripts/

# Use TUNA package repository.
RUN ./scripts/tuna.py && apt-get --assume-yes update

# Install all dependencies required by WaveJudge.
RUN ./scripts/deps-install.py

# Step 3: copy the application from `build` to this final image.
WORKDIR /app
COPY --from=build /app/target/$profile/* ./

ENTRYPOINT ["wave_judge"]
