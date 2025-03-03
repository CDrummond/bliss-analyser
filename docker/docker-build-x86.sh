#!/bin/bash
## #!/usr/bin/env bash
set -eux

uname -a
DESTDIR=/src/releases

mkdir -p $DESTDIR/bin
rm -rf $DESTDIR/bin/*

export RUST_BACKTRACE=full
cargo build --release --features=libav

strip /build/release/bliss-analyser && cp /build/release/bliss-analyser $DESTDIR/bliss-analyser-x86-ffmpeg5

cp UserGuide.md $DESTDIR/README.md
cp LICENSE $DESTDIR/
cp configs/linux.ini $DESTDIR/config.ini
cp scripts/bliss-analyser-arm $DESTDIR/bliss-analyser
