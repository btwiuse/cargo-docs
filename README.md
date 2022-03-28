cargo-docs
==========

[![crates.io](https://img.shields.io/crates/v/cargo-docs.svg)](https://crates.io/crates/cargo-docs)
[![Documentation](https://docs.rs/cargo-docs/badge.svg)](https://docs.rs/cargo-docs)
[![Build Status](https://travis-ci.org/btwiuse/cargo-docs.svg?branch=master)](https://travis-ci.org/btwiuse/cargo-docs)

A cargo plugin for serving crate doc locally.

# Usage

```
$ cargo docs
Generating documentation for crate...
Serving on http://127.0.0.1:8080
```

passthrough `cargo doc` options after --
```
$ cargo docs -- -q
Generating documentation for crate...
Serving on http://127.0.0.1:8080
```
