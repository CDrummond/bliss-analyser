# Bliss Analyser

Simple rust app to analyse songs with [bliss-rs](https://github.com/Polochon-street/bliss-rs).
The output of this is a SQLite database containing song metadata and
bliss analysis. This is then intended to be used by [Bliss Mixer](https://github.com/CDrummond/bliss-mixer)


# Building

This application can be built in 3 variants:

1. Using command-line `ffmpeg` to decode files
2. Using `libavcodec`, etc, to decode files
3. Using `libavcodec`, etc, to decode files, but statically linked to `libavcodec`, etc.


Using `libavcodec` is about 70% faster, but might have issues with library, versioning, etc.
Using `libavcodec` statically linked my reduce supported file formats.
Using `ffmpeg` whilst slower produces a more portable application.

## Build for 'ffmpeg' command-line usage

`clang` and `pkg-config` are required to build, as well as
[Rust](https://www.rust-lang.org/tools/install)

To install dependencies on a Debian system:

```
apt install -y clang pkg-config
```

To install dependencies on a Fedora system:
```
dnf install clang pkg-config
```

Build with `cargo build --release --features=ffmpeg`

`ffmpeg` is then a run-time dependency, and should be installed on any system where this application
is to be run - it should also be in the users `$PATH`


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

Build with `cargo build --release --features=libav,libavstatic`


# Usage

Please refer to `UserGuide.md` for details of how this tool may be used.
