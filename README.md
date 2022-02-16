# Bliss Analyser

Simple rust app to analyse songs with [bliss-rs](https://github.com/Polochon-street/bliss-rs).
The output of this is a SQLite database containing song metadata and
bliss analysis. This is then intended to be used by [Bliss Mixer](https://github.com/CDrummond/bliss-mixer)


# Building

clang, pkg-config, and ffmpeg are required to build, as well as
[Rust](https://www.rust-lang.org/tools/install)

To install dependencies on a Debian system:

```
apt install -y clang libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev pkg-config
```

Build with `cargo build --release`


## Analyse music library

```
$ bliss-analyse --db /path/to/music
```

