#!/bin/bash
## #!/usr/bin/env bash
set -eux

uname -a
DESTDIR=/src/releases

for d in armhf-linux aarch64-linux; do
    mkdir -p $DESTDIR/$d
    rm -rf $DESTDIR/$d/*
done

function build {
	echo Building for $1 to $3...

	if [[ ! -f /build/$1/release/bliss-analyser ]]; then
		export RUST_BACKTRACE=full
		export PKG_CONFIG=${1//unknown-/}-pkg-config
		BINDGEN_EXTRA_CLANG_ARGS="--sysroot /usr/${1//unknown-/}" cargo build --release --target $1
	fi

	$2 /build/$1/release/bliss-analyser && cp /build/$1/release/bliss-analyser $DESTDIR/$3
}

build arm-unknown-linux-gnueabihf arm-linux-gnueabihf-strip armhf/bliss-analyser
build aarch64-unknown-linux-gnu aarch64-linux-gnu-strip aarch64/bliss-analyser

