FROM --platform=linux/arm64 rust:1.89.0-bookworm@sha256:e090f7b4adf86191313dba91260351d7f5e15cac0fe34f26706a805c0cb9641f

RUN curl -L https://github.com/tttapa/docker-arm-cross-toolchain/releases/download/1.0.0/x-tools-aarch64-rpi3-linux-gnu-armv6-rpi-linux-gnueabihf-gcc12.tar.xz \
    | tar -xJ -C /opt

RUN echo "set(CMAKE_SYSROOT /sysroot)" \
    >> /opt/x-tools/armv6-rpi-linux-gnueabihf/armv6-rpi-linux-gnueabihf.toolchain.cmake

RUN apt-get update \
  && apt-get install -y \
      gcc-arm-linux-gnueabihf \
      g++-arm-linux-gnueabihf \
      cmake \
      pkg-config \
  && rm -rf /var/lib/apt/lists/*

RUN rustup target add arm-unknown-linux-gnueabihf
