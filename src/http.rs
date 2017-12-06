use std::io::{Read, Error, ErrorKind};
use futures::Stream;
use tokio_core::reactor::Core;
use hyper;
use hyper_tls::HttpsConnector;

pub fn get<S: AsRef<str>>(url: S) -> Result<Response, hyper::Error> {
    let uri: hyper::Uri = url.as_ref().parse()?;
    let mut core = Core::new()?;
    let handle = core.handle();
    match uri.scheme() {
        Some("http") => {
            let client = hyper::Client::new(&handle);
            let res = core.run(client.get(uri.clone()))?;
            Ok(Response { res, core })
        },
        Some("https") => {
            let client = hyper::Client::configure()
                .connector(HttpsConnector::new(1, &handle)
                           .map_err(|_| hyper::Error::Incomplete)?)
                .build(&handle);
            let res = core.run(client.get(uri.clone()))?;
            Ok(Response { res, core })
        },
        _ =>
            Err(hyper::Error::Version),
    }
}

pub struct Response {
    res: hyper::Response,
    core: Core,
}

impl Response {
    pub fn status(&self) -> hyper::StatusCode {
        self.res.status()
    }

    pub fn headers(&self) -> &hyper::Headers {
        self.res.headers()
    }

    pub fn body(self) -> Body {
        Body {
            body: Some(self.res.body()),
            core: self.core,
            buf_offset: 0,
            buf: vec![],
        }
    }
}

pub struct Body {
    body: Option<hyper::Body>,
    core: Core,
    buf_offset: usize,
    buf: Vec<u8>,
}

impl Body {
    fn recv_next(&mut self) -> Result<Vec<u8>, hyper::Error> {
        let body_future = self.body.take()
            .expect("body")
            .into_future();
        let (next_item, body) = self.core.run(body_future)
            .map_err(|(e, _body)| e)?;
        self.body = Some(body);
        match next_item {
            None => Ok(vec![]),
            Some(ref buf) if buf.len() == 0 =>
                self.recv_next(),
            Some(buf) =>
                Ok(buf.to_vec()),
        }
    }
}


impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let err_mapper = |e| Error::new(ErrorKind::Other, e);
        if self.buf_offset >= self.buf.len() {
            self.buf = self.recv_next()
                .map_err(err_mapper)?;
            self.buf_offset = 0;
        }

        let buf_left = self.buf.len() - self.buf_offset;
        let size = buf.len().min(buf_left);
        if size > 0 {
            buf[0..size].copy_from_slice(
                &self.buf[self.buf_offset..(self.buf_offset + size)]
            );
            self.buf_offset += size;
        }
        Ok(size)
    }
}
