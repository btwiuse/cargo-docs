FROM btwiuse/arch:rustup

WORKDIR /app

ADD . /app/

RUN cargo build --release

RUN cp ./target/release/cargo-* /usr/local/bin/

CMD cargo docs -b
