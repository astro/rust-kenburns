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

    pub fn body(mut self) -> Result<Vec<u8>, hyper::Error> {
        let mut body = vec![];
        self.core.run(
            self.res.body().for_each(|chunk| {
                body.extend(chunk);
                Ok(())
            })
        )?;
        Ok(body)
    }
}
