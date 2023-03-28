cargo-docs
==========

[![crates.io](https://img.shields.io/crates/v/cargo-docs.svg)](https://crates.io/crates/cargo-docs)
[![Documentation](https://docs.rs/cargo-docs/badge.svg)](https://docs.rs/cargo-docs)
[![Build Status](https://travis-ci.org/btwiuse/cargo-docs.svg?branch=master)](https://travis-ci.org/btwiuse/cargo-docs)

A cargo plugin for serving rust and crate doc locally.

```
$ cargo docs --help
Usage: cargo docs [OPTIONS] [EXTRA_ARGS]...

Arguments:
  [EXTRA_ARGS]...  Passthrough extra args to `cargo doc`

Options:
      --host <HOST>                    Set host [env: HOST=] [default: 127.0.0.1]
  -p, --port <PORT>                    Set port [env: PORT=] [default: 8080]
  -r, --random-port                    Use random port [env: CARGO_DOCS_RANDOM_PORT=true]
  -s, --search <ITEM>                  Search for item
  -d, --dir <DIR>                      Serve directory content [env: DIR=]
  -c, --manifest-path <MANIFEST_PATH>  Crate manifest path [default: Cargo.toml]
  -w, --watch                          Re-generate doc on change [env: CARGO_DOCS_WATCH=]
  -o, --open                           Open in browser [env: CARGO_DOCS_OPEN=true]
  -b, --book                           Serve rust book and std doc instead
  -h, --help                           Print help information
  -V, --version                        Print version information
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
[INFO] Serving crate doc on http://127.0.0.1:45669
[INFO] Opening http://127.0.0.1:45669
```

Same as above plus automatically rebuild and reload on file changes.
```
$ cargo docs -row
[INFO] Listening for changes...
[INFO] Serving crate doc on http://127.0.0.1:45669
[INFO] Opening http://127.0.0.1:45669
```

Serve rust docs instead (roughly the same as [`cargo-book`](https://crates.io/crates/cargo-book))
```
$ cargo docs -bro
[INFO] Serving rust doc on http://127.0.0.1:46661
[INFO] Opening http://127.0.0.1:46661
```

Search for `SocketAddr` in rust std doc served on random port and open it in browser
```
$ cargo docs -bros SocketAddr
[INFO] Serving rust doc on http://127.0.0.1:40143
[INFO] Opening http://127.0.0.1:40143/std/?search=SocketAddr
```

## Pro Tips

Passthrough `cargo doc` options after --
```
$ cargo docs -- --quiet
[INFO] Running cargo doc --quiet
[INFO] Serving crate doc on http://127.0.0.1:8080
```

If you are on WSL2, set `BROWSER=/mnt/c/Path/To/Your/Browser.exe` environment variable to open in desktop browser
```
$ export BROWSER="/mnt/c/Program Files/Firefox Nightly/firefox.exe"
```

Tired of typing `-o`, `-ro`, `-row`? Try these environment variables to save you some key strokes.
```
$ export CARGO_DOCS_OPEN=true
$ export CARGO_DOCS_WATCH=true
$ export CARGO_DOCS_RANDOM_PORT=true
```

`cargo-book` relies on presence of the rust-docs component
```
$ rustup component add rust-docs
```

try in codespaces
