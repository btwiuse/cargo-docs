cargo-docs
==========

[![crates.io](https://img.shields.io/crates/v/cargo-docs.svg)](https://crates.io/crates/cargo-docs)
[![Documentation](https://docs.rs/cargo-docs/badge.svg)](https://docs.rs/cargo-docs)
[![Build Status](https://travis-ci.org/btwiuse/cargo-docs.svg?branch=master)](https://travis-ci.org/btwiuse/cargo-docs)

A cargo plugin for serving rust and crate doc locally.

By default, it will call `cargo doc` to build crate doc and start a local server.

Add `--book` option to see rust doc instead.

# Usage

Serve crate doc on local port 8080
```
$ cargo docs -p 8080
Serving crate doc on http://127.0.0.1:8080
```

Passthrough `cargo doc` options after --
```
$ cargo docs -p 8080 -- --quiet
Serving crate doc on http://127.0.0.1:8080
```

Serve rust doc (effectively the same as [cargo-book](https://crates.io/crates/cargo-book)
```
$ cargo docs --book
Serving rust doc on http://127.0.0.1:8080
```

Serve current directory content on 0.0.0.0:8910 (listing is not supported yet)
```
$ cargo docs -d . --host 0.0.0.0 --port 8910
Serving . on http://0.0.0.0:8910
```
