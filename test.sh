#!/bin/bash
export MOZTOOLS_PATH='C:\mozilla-build\msys\bin;C:\mozilla-build\bin'
export AUTOCONF="C:/mozilla-build/msys/local/bin/autoconf-2.13"
export LINKER="lld-link.exe"
export CC="clang-cl.exe"
export CXX="clang-cl.exe"
export NATIVE_WIN32_PYTHON="C:\\mozilla-build\\python2\\python.exe"
export PYTHON3="C:\\mozilla-build\\python3\\python3.exe"
export LIBCLANG_PATH="C:\\ProgramData\\scoop\\apps\\llvm\\current\\lib"
cargo build --verbose --features debugmozjs