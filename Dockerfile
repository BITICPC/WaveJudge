# Step 1: Build application
FROM rust:1.40-stretch as build
ARG profile=release
ARG tuna=yes
WORKDIR /app
# Copy all application related files and directories into the image.
COPY ./ ./
RUN ./docker/use-tuna.py --tuna $tuna && apt update
# Install necessary dependencies to build WaveJudge from source.
RUN apt install openssl libseccomp2-dev
# And then build
RUN ./build.py --profile $profile

# Step 2: Build application runtime based on a fresh debian image
FROM debian:stretch-slim as runtime
WORKDIR /deps

# Use tuna source if necessary.
COPY docker/ ./scripts/
RUN ./scripts/deps-install.py --use-tuna $tuna

# Step 3: copy the application from `build` to this final image.
WORKDIR /app
COPY --from=build /app/target/$profile/* ./

ENTRYPOINT ["wave_judge"]
