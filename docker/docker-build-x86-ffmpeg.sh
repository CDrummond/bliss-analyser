#!/bin/bash
## #!/usr/bin/env bash
set -eux

uname -a
DESTDIR=/src/releases
mkdir -p $DESTDIR

function build {
	echo Building for $1 to $3...

    if [[ ! -f /build/$1/release/bliss-analyser ]]; then
        cargo build --release --features=update-aubio-bindings,libav,staticlibav --target $1
    fi

    $2 /build/$1/release/bliss-analyser && cp /build/$1/release/bliss-analyser $DESTDIR/$3
}

build x86_64-unknown-linux-musl strip bliss-analyser

cp UserGuide.md $DESTDIR/README.md
cp LICENSE $DESTDIR/
cp configs/linux.ini $DESTDIR/config.ini
