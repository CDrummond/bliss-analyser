#!/bin/bash
## #!/usr/bin/env bash
set -eux

uname -a
DESTDIR=/src/releases

mkdir -p $DESTDIR/bin
rm -rf $DESTDIR/bin/*

function build {
	echo Building for $1 to $3...

    if [[ ! -f /build/$1/release/bliss-analyser ]]; then
        cargo build --release --features=libav,staticlibav --target $1
    fi

    $2 /build/$1/release/bliss-analyser && cp /build/$1/release/bliss-analyser $DESTDIR/$3
}

build arm-unknown-linux-gnueabihf arm-linux-gnueabihf-strip bin/bliss-analyser-armhf
build aarch64-unknown-linux-gnu aarch64-linux-gnu-strip bin/bliss-analyser-aarch64

cp UserGuide.md $DESTDIR/README.md
cp LICENSE $DESTDIR/
cp configs/linux.ini $DESTDIR/config.ini
cp scripts/bliss-analyser-arm $DESTDIR/bliss-analyser
