FROM ubuntu:22.04
# ubuntu 22.04 is called jammy

# add arch
RUN sed 's/^deb http/deb [arch=amd64] http/' -i '/etc/apt/sources.list'
RUN echo 'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-updates main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-backports main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-security main universe multiverse restricted' >> /etc/apt/sources.list
RUN dpkg --add-architecture arm64
# add for actions
RUN apt update && apt install -y curl git && apt-get clean
# add main build deps
RUN apt install --no-install-recommends -y build-essential pkg-config m4 python3 python3-distutils llvm llvm-dev lld libclang-dev clang && apt-get clean
# add cross deps
RUN apt install --no-install-recommends -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu qemu-user qemu-user-static && apt-get clean
# add runtime deps
RUN apt install --no-install-recommends -y libc6:arm64 libstdc++6:arm64 && apt-get clean
# set runner to qemu
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER="/usr/bin/qemu-aarch64"
# set linker to cross linker
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="/usr/bin/aarch64-linux-gnu-gcc"

# use clang compiler
RUN echo '#!/bin/bash \nclang -target aarch64-unknown-linux-gnu -isysroot=/usr/aarch64-linux-gnu --sysroot=/ -Wno-unused-command-line-argument -fuse-ld=lld "$@"' > /bin/clang-aarch64 && chmod +x /bin/clang-aarch64
RUN echo '#!/bin/bash \nclang++ -target aarch64-unknown-linux-gnu -isysroot=/usr/aarch64-linux-gnu --sysroot=/ -Wno-unused-command-line-argument -fuse-ld=lld "$@"' > /bin/clang++-aarch64 && chmod +x /bin/clang++-aarch64
ENV CC="clang-aarch64"
ENV CXX="clang++-aarch64"
ENV BINDGEN_EXTRA_CLANG_ARGS="-isysroot=/usr/aarch64-linux-gnu --sysroot=/ -Wno-unused-command-line-argument -fuse-ld=lld"
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="clang-aarch64"