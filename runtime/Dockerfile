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
RUN ln -s /deps/python35/bin/python3.5 /usr/bin/python3.5 && \
    ln -s /deps/python36/bin/python3.6 /usr/bin/python3.6 && \
    ln -s /deps/python37/bin/python3.7 /usr/bin/python3.7 && \
    ln -s /deps/python38/bin/python3.8 /usr/bin/python3.8
# Make the default python version to be python3.8
RUN ln -s ./python3.8 /usr/bin/python3

# Use TUNA package repository.
RUN apt-get --assume-yes update && \
    apt-get --assume-yes install apt-transport-https && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch main contrib non-free" \
        > /etc/apt/sources.list && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-updates main contrib non-free" \
        >> /etc/apt/sources.list && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-backports main contrib non-free" \
        >> /etc/apt/sources.list && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian-security stretch/updates main contrib non-free" \
        >> /etc/apt/sources.list && \
    apt-get --assume-yes update

# Install CXX environments.
RUN apt-get --assume-yes install gcc g++ clang

# Download Java archive, extract it and install java environments to /usr/bin.
WORKDIR /deps/java
ADD https://download.java.net/java/GA/jdk13.0.1/cec27d702aa74d5a8630c65ae61e4305/9/GPL/openjdk-13.0.1_linux-x64_bin.tar.gz \
    ./
RUN tar -xzf openjdk-13.0.1_linux-x64_bin.tar.gz && \
    rm openjdk-13.0.1_linux-x64_bin.tar.gz && \
    ln -s /deps/java/jdk-13.0.1/bin/java /usr/bin/java && \
    ln -s /deps/java/jdk-13.0.1/bin/javac /usr/bin/javac && \
    ln -s /deps/java/jdk-13.0.1/bin/jar /usr/bin/jar

# Install rust environments.
WORKDIR /deps/rust
ENV RUSTUP_HOME=/deps/rust/rustup CARGO_HOME=/deps/rust/cargo
RUN apt-get --assume-yes install curl && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN $CARGO_HOME/bin/rustup toolchain install 1.38.0 1.39.0 1.40.0
