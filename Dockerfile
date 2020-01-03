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


# Step 2: Build application runtime.
FROM lancern/wave-judge-runtime:latest AS final
WORKDIR /app
COPY --from=build /app/target/$profile ./

ENTRYPOINT ["wave_judge"]
