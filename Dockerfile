# Step 1: Build application
FROM lancern/libseccomp:2.4-stretch AS libseccomp
FROM lancern/openssl:1.1.1-stretch AS openssl
FROM rust:1.40-stretch AS build

# Copy the prebuilt libseccomp and openssl binaries from corresponding base images.
COPY --from=libseccomp /libseccomp /libseccomp
COPY --from=openssl /openssl /openssl

# Build WaveJudge itself.
WORKDIR /app
COPY ./ ./
ARG profile=release
ENV LIBSECCOMP_LIB_PATH=/libseccomp/lib LIBSECCOMP_LIB_TYPE=static
ENV OPENSSL_DIR=/openssl/install OPENSSL_STATIC=yes
RUN ./build.py --profile $profile


# Step 2: Build application runtime based on a fresh debian image
FROM lancern/python:3.5.9-stretch AS python35
FROM lancern/python:3.6.10-stretch AS python36
FROM lancern/python:3.7.6-stretch AS python37
FROM lancern/python:3.8.1-stretch AS python38
FROM debian:stretch-slim AS runtime
WORKDIR /deps

# Copy python installations from corresponding python distribution images.
COPY --from=python35 /python ./python35
COPY --from=python36 /python ./python36
COPY --from=python37 /python ./python37
COPY --from=python38 /python ./python38
# And make symbolic links in /usr/local/bin to the corresponding python executables.
RUN ln -s ./python35/bin/python3.5 /usr/bin/python3.5 && \
    ln -s ./python36/bin/python3.6 /usr/bin/python3.6 && \
    ln -s ./python37/bin/python3.7 /usr/bin/python3.7 && \
    ln -s ./python38/bin/python3.8 /usr/bin/python3.8

COPY docker ./scripts/

# Use TUNA package repository.
RUN apt-get --assume-yes update && \
    ./scripts/tuna.py && \
    apt-get --assume-yes update

# Install all other dependencies required by WaveJudge.
RUN ./scripts/deps-install.py

# Step 3: copy the application from `build` to this final image.
WORKDIR /app
COPY --from=build /app/target/$profile ./

ENTRYPOINT ["wave_judge"]
