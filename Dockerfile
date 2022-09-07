FROM btwiuse/arch:rustup

WORKDIR /app

ADD . /app/

RUN cargo build --release

RUN cp ./target/release/cargo-* /usr/local/bin/

ENV HOST=0.0.0.0

CMD cargo docs -b
