# Bliss Analyser

Simple rust app to analyse songs with [bliss-rs](https://github.com/Polochon-street/bliss-rs).
The output of this is a SQLite database containing song metadata and
bliss analysis. This is then intended to be used by [Bliss Mixer](https://github.com/CDrummond/bliss-mixer)


# Building

This application can be built in 4 variants:

1. Using `libavcodec`, etc, to decode files.
2. Using `libavcodec`, etc, to decode files, but statically linked to `libavcodec`, etc.
3. Using `symphonia` to decode files.


`libavcodec` is the fastest (~15% faster than `symphonia`), but might have issues with
library, versioning, etc., unless these libraries are statically linked in. `libavcodec`
statically linked may reduce supported file formats, but is more portable.
file formats, but is more portable.

`symphonia` also produces a more portable application, is only slightly slower to decode
files, but has more limited codec support, and can fail on more files.


## Build for 'libavcodec' library usage

`clang`, `pkg-config`, and `ffmpeg` are required to build, as well as
[Rust](https://www.rust-lang.org/tools/install)

To install dependencies on a Debian system:

```
apt install -y clang libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev pkg-config
```

To install dependencies on a Fedora system:
```
dnf install ffmpeg-devel clang pkg-config
```

Build with `cargo build --release --features=libav`

If building on a Raspberry Pi, then `rpi` also needs to be passed to `--features`, e.g.
`cargo build --release --features=libav,rpi`

The resultant application will be less portable, due to dependencies on `libavcodec` libraries (and
their dependencies).



## Build for 'libavcodec' library usage, statically linked

`clang`, `pkg-config`, and `ffmpeg` are required to build, as well as
[Rust](https://www.rust-lang.org/tools/install)

To install dependencies on a Debian system:

```
apt install -y clang libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev pkg-config
```

To install dependencies on a Fedora system:
```
dnf install ffmpeg-devel clang pkg-config
```

Build with `cargo build --release --features=libav,staticlibav`

If building on a Raspberry Pi, then `rpi` also needs to be passed to `--features`, e.g.
`cargo build --release --features=libav,staticlibav,rpi`



## Build for 'symphonia'

`clang`, and `pkg-config` are required to build, as well as
[Rust](https://www.rust-lang.org/tools/install)

To install dependencies on a Debian system:

```
apt install -y clang pkg-config
```

To install dependencies on a Fedora system:
```
dnf install clang pkg-config
```

Build with `cargo build --release --features=symphonia`




# Usage

Please refer to `UserGuide.md` for details of how this tool may be used.
