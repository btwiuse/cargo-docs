use bytes::{Buf, Bytes, BytesMut};
use futures::Async;
use tower_web::util::BufStream;

pub struct ReplacingBufStream<Stream: BufStream<Item = std::io::Cursor<Bytes>>> {
    stream: Stream,
    to_be_replaced: Bytes,
    replacement: Bytes,
    bufs: Vec<std::io::Cursor<Bytes>>,
}

impl<Stream: BufStream<Item = std::io::Cursor<Bytes>>> ReplacingBufStream<Stream> {
    pub fn new(
        inner: Stream,
        to_be_replaced: Bytes,
        replacement: Bytes,
    ) -> ReplacingBufStream<Stream> {
        ReplacingBufStream {
            stream: inner,
            to_be_replaced,
            replacement,
            bufs: Vec::new(),
        }
    }
}

impl<Stream: BufStream<Item = std::io::Cursor<Bytes>>> BufStream for ReplacingBufStream<Stream>
where
    <Stream as tower_web::util::BufStream>::Error: std::fmt::Debug,
{
    type Item = std::io::Cursor<bytes::Bytes>;
    type Error = Stream::Error;

    fn poll(&mut self) -> futures::Poll<Option<Self::Item>, Self::Error> {
        match non_matching_prefix_length(&self.bufs, &self.to_be_replaced.as_ref()) {
            Action::Return(size) => {
                if self.bufs[0].bytes().len() == size {
                    Ok(Async::Ready(Some(self.bufs.remove(0))))
                } else {
                    let buf = self.bufs.remove(0);
                    let mut ret = buf.into_inner();
                    self.bufs
                        .insert(0, std::io::Cursor::new(ret.split_off(size)));
                    Ok(Async::Ready(Some(std::io::Cursor::new(ret))))
                }
            }
            Action::Replace => {
                let mut len = 0;
                let mut buf = None;
                while len < self.to_be_replaced.len() {
                    let buf2 = self.bufs.remove(0);
                    len += buf2.bytes().len();
                    buf = Some(buf2)
                }
                let buf = buf.unwrap();
                if len > self.to_be_replaced.len() {
                    let mut inner = buf.into_inner();
                    self.bufs.insert(
                        0,
                        std::io::Cursor::new(
                            inner.split_off(inner.len() + self.to_be_replaced.len() - len),
                        ),
                    );
                }
                Ok(Async::Ready(Some(std::io::Cursor::new(
                    self.replacement.clone(),
                ))))
            }
            Action::Read => match self.stream.poll() {
                Ok(Async::Ready(Some(buf))) => {
                    self.bufs.push(buf);
                    self.poll()
                }
                Ok(Async::Ready(None)) => {
                    if !self.bufs.is_empty() {
                        return Ok(Async::Ready(Some(self.bufs.remove(0))));
                    }
                    Ok(Async::Ready(None))
                }
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Err(err) => Err(err),
            },
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Action {
    Return(usize),
    Replace,
    Read,
}

pub struct FreezingBufStream<Stream: BufStream<Item = std::io::Cursor<BytesMut>>>(pub Stream);

impl<Err, Stream: BufStream<Item = std::io::Cursor<BytesMut>, Error = Err>> BufStream
    for FreezingBufStream<Stream>
{
    type Item = std::io::Cursor<Bytes>;
    type Error = Err;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        match self.0.poll() {
            Ok(Async::Ready(Some(bytes))) => {
                let position = bytes.position() as usize;
                let bytes = bytes.into_inner();
                Ok(Async::Ready(Some(std::io::Cursor::new(
                    bytes.freeze().split_off(position),
                ))))
            }
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err),
        }
    }
}

fn non_matching_prefix_length<B: Buf>(bufs: &[B], prefix: &[u8]) -> Action {
    if bufs.is_empty() {
        return Action::Read;
    }
    if bufs[0].bytes().len() < prefix.len() {
        if prefix.starts_with(&bufs[0].bytes()) {
            return match non_matching_prefix_length(&bufs[1..], &prefix[bufs[0].bytes().len()..]) {
                Action::Return(_) => Action::Return(bufs[0].bytes().len()),
                Action::Replace => Action::Replace,
                Action::Read => Action::Read,
            };
        }
        let mut ret = 0;
        while !prefix.starts_with(&bufs[0].bytes()[ret..]) && ret < bufs[0].bytes().len() {
            ret += 1;
        }
        return Action::Return(ret);
    } else {
        if bufs[0].bytes().starts_with(prefix) {
            return Action::Replace;
        }
        let mut ret = 0;
        while !bufs[0].bytes()[ret..]
            .starts_with(&prefix[..std::cmp::min(prefix.len(), bufs[0].bytes().len() - ret)])
        {
            ret += 1;
        }
        return Action::Return(ret);
    }
}

#[cfg(test)]
mod tests {
    use super::{non_matching_prefix_length, Action, FreezingBufStream, ReplacingBufStream};
    use bytes::{Bytes, BytesMut};
    use futures::Future;
    use std::path::PathBuf;
    use tokio::runtime::Runtime;
    use tower_web::util::BufStream;

    const PREFIX: &'static [u8] = b"matches";

    #[test]
    fn no_bufs() {
        let bufs = make_bufs(&[]);
        assert_eq!(Action::Read, non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn one_buf_is_prefix() {
        let bufs = make_bufs(&["matches"]);
        assert_eq!(Action::Replace, non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn one_buf_is_prefix_start() {
        let bufs = make_bufs(&["mat"]);
        assert_eq!(Action::Read, non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn one_buf_is_not_prefix_shorter() {
        let bufs = make_bufs(&["nope"]);
        assert_eq!(Action::Return(4), non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn one_buf_is_not_prefix_longer() {
        let bufs = make_bufs(&["nopenopenope"]);
        assert_eq!(
            Action::Return(12),
            non_matching_prefix_length(&bufs, PREFIX)
        );
    }

    #[test]
    fn one_buf_starts_with_prefix() {
        let bufs = make_bufs(&["matchesyesyesyes"]);
        assert_eq!(Action::Replace, non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn one_buf_contains_prefix() {
        let bufs = make_bufs(&["yesmatchesyesyes"]);
        assert_eq!(Action::Return(3), non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn one_buf_same_length_ends_prefix() {
        let bufs = make_bufs(&["yesmatc"]);
        assert_eq!(Action::Return(3), non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn two_buf_exact_match() {
        let bufs = make_bufs(&["mat", "ches"]);
        assert_eq!(Action::Replace, non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn two_buf_with_suffix() {
        let bufs = make_bufs(&["mat", "chesyes"]);
        assert_eq!(Action::Replace, non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn two_buf_with_prefix() {
        let bufs = make_bufs(&["yesmat", "ches"]);
        assert_eq!(Action::Return(3), non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn two_buf_wrong_ending() {
        let bufs = make_bufs(&["mat", "chnope"]);
        assert_eq!(Action::Return(3), non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn two_buf_prefix_and_wrong_ending() {
        let bufs = make_bufs(&["nomat", "chnope"]);
        assert_eq!(Action::Return(2), non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn three_bufs() {
        let bufs = make_bufs(&["mat", "ch", "es"]);
        assert_eq!(Action::Replace, non_matching_prefix_length(&bufs, PREFIX));
    }

    #[test]
    fn stream() {
        use tower_web::util::BufStream;
        let stream = ReplacingBufStream::new(
            Bytes::from("foobarbaz"),
            Bytes::from("bar"),
            Bytes::from("blammo"),
        );
        let res = stream.collect::<Vec<u8>>().wait().unwrap();
        assert_eq!(res, "fooblammobaz".as_bytes().to_vec())
    }

    #[test]
    fn stream_trailing_prefix() {
        use tower_web::util::BufStream;
        let stream = ReplacingBufStream::new(
            Bytes::from("foobarba"),
            Bytes::from("bar"),
            Bytes::from("blammo"),
        );
        let res = stream.collect::<Vec<u8>>().wait().unwrap();
        assert_eq!(res, "fooblammoba".as_bytes().to_vec())
    }

    #[test]
    fn large_file() {
        let needle = "https://doc.rust-lang.org/nightly";
        let replacement = "http://127.0.0.1:8080";

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("testdata");
        path.push("struct.Group.html");

        let want = std::fs::read_to_string(&path)
            .unwrap()
            .replace(needle, replacement)
            .as_bytes()
            .to_owned();

        let stream = FreezingBufStream(tokio_fs::File::from_std(
            std::fs::File::open(&path).unwrap(),
        ));
        let stream = ReplacingBufStream::new(stream, Bytes::from(needle), Bytes::from(replacement));
        let res = Runtime::new()
            .unwrap()
            .block_on(stream.collect::<Vec<u8>>())
            .unwrap();
        assert_eq!(want, res)
    }

    fn make_bufs(strs: &[&str]) -> Vec<std::io::Cursor<BytesMut>> {
        let mut bufs = Vec::with_capacity(strs.len());
        for s in strs {
            bufs.push(std::io::Cursor::new(BytesMut::from(s.as_bytes())));
        }
        bufs
    }
}
