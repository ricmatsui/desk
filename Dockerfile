FROM balenalib/raspberry-pi-debian:buster

RUN apt-get update && apt-get install -y \
    libx11-dev \
    libxcursor-dev \
    libxinerama-dev \
    libxrandr-dev \
    libxi-dev \
    libasound2-dev \
    libudev-dev \
    mesa-common-dev \
    libgl1-mesa-dev \
    libraspberrypi-dev \
    raspberrypi-kernel-headers \
    build-essential \
    pkg-config \
    wget \
    git \
    vim \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /root

RUN wget https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabihf/rustup-init \
    && chmod u+x rustup-init \
    && ./rustup-init -y
