cargo-docs
==========

[![crates.io](https://img.shields.io/crates/v/cargo-docs.svg)](https://crates.io/crates/cargo-docs)
[![Documentation](https://docs.rs/cargo-docs/badge.svg)](https://docs.rs/cargo-docs)
[![Build Status](https://travis-ci.org/btwiuse/cargo-docs.svg?branch=master)](https://travis-ci.org/btwiuse/cargo-docs)

A cargo plugin for serving rust and crate doc locally.

```
$ cargo docs --help
cargo-docs-docs

USAGE:
    cargo-docs docs [OPTIONS] [EXTRA_ARGS]...

ARGS:
    <EXTRA_ARGS>...    Passthrough extra args to `cargo doc`

OPTIONS:
    -b, --book                             Serve rust book and std doc instead
    -c, --manifest-path <MANIFEST_PATH>    Crate manifest path [default: Cargo.toml]
    -d, --dir <DIR>                        Serve directory content [env: DIR=]
    -h, --help                             Print help information
        --host <HOST>                      Set host [env: HOST=] [default: 127.0.0.1]
    -o, --open                             Open in browser
    -p, --port <PORT>                      Set port [env: PORT=] [default: 8080]
    -r, --random-port                      Use random port
    -w, --watch                            Re-generate doc on change TODO: unimplemented
```

By default, it will call `cargo doc` to build crate doc and start a local server.

Add `--book` option to see rust doc instead.

## Install

```
$ cargo install cargo-docs
```

## Examples

Serve crate doc on random port and open in browser  
```
$ cargo docs -ro
Serving crate doc on http://127.0.0.1:45669
Opening http://127.0.0.1:45669
```

Serve rust doc (effectively the same as [cargo-book](https://crates.io/crates/cargo-book)) on random port and open in browser
```
$ cargo docs -bro
Serving rust doc on http://127.0.0.1:46661
Opening http://127.0.0.1:46661
```

Search for `SocketAddr` in rust std doc served on random port and open it in browser
```
$ cargo docs -bros SocketAddr
Serving rust doc on http://127.0.0.1:40143
Opening http://127.0.0.1:40143/std/?search=SocketAddr
```

## Pro Tips

Passthrough `cargo doc` options after --
```
$ cargo docs -- --quiet
Running cargo doc --quiet
Serving crate doc on http://127.0.0.1:8080
```

If you are on WSL2, set `BROWSER=/mnt/c/Path/To/Your/Browser.exe` environment variable to open in desktop browser
```
$ export BROWSER="/mnt/c/Program Files/Firefox Nightly/firefox.exe"
```

Tired of typing `-o`, `-ro`? Set these environment variables to save you some key strokes.
```
$ export CARGO_DOCS_OPEN=1
$ export CARGO_DOCS_RANDOM_PORT=1
```
