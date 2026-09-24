#!/bin/sh
set -eu

# Native development dependencies for the Rust v2 build.
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libudev-dev \
    libv4l-dev
