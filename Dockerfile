# Step 1: Build application
FROM rust:1.40-stretch as build
ARG profile=release
ARG tuna=yes
WORKDIR /app
# Copy all application related files and directories into the image.
COPY ./ ./
# Configure package source to TUNA.
RUN ./docker/use-tuna.py --tuna $tuna && apt-get --assume-yes update
# Install the build-essential meta package.
RUN apt-get --assume-yes install build-essential
# Build libseccomp and openssl
WORKDIR /app/libseccomp
RUN ./autogen.sh && ./configure && make && make install
WORKDIR /app/openssl
RUN ./config && make && make install
# And then build WaveJudge itself.
WORKDIR /app
RUN ./build.py --profile $profile

# Step 2: Build application runtime based on a fresh debian image
FROM debian:stretch-slim as runtime
WORKDIR /deps

# Install python3.
RUN apt-get --assume-yes update && apt-get --assume-yes install python3

# Install all dependencies required by WaveJudge.
COPY docker/ ./scripts/
RUN ./scripts/deps-install.py --tuna $tuna

# Step 3: copy the application from `build` to this final image.
WORKDIR /app
COPY --from=build /app/target/$profile/* ./

ENTRYPOINT ["wave_judge"]
