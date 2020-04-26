mod binding;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use js_sys::{Promise, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use binding::*;

pub struct Client;

pub struct ResponseFuture {
    inner: JsFuture,
}

pub struct Body {
    inner: Option<BodyInner>,
}

struct BodyInner {
    reader: ReadableStreamDefaultReader,
    chunk: JsFuture,
}

impl Client {
    pub fn request<B: AsRef<[u8]>>(&self, req: http::Request<B>) -> ResponseFuture {
        let (parts, body) = req.into_parts();
        self.request_(http::Request::from_parts(parts, body.as_ref()))
    }

    fn request_(&self, req: http::Request<&[u8]>) -> ResponseFuture {
        let promise = match convert_request(req) {
            Ok(req) => web_sys::window().unwrap().fetch_with_request(&req),
            Err(e) => Promise::reject(&e),
        };
        let inner = JsFuture::from(promise);
        ResponseFuture { inner }
    }
}

impl<B: AsRef<[u8]>> tower_service::Service<http::Request<B>> for Client {
    type Response = http::Response<Body>;
    type Error = JsValue;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        self.request(req)
    }
}

impl Future for ResponseFuture {
    type Output = Result<http::Response<Body>, JsValue>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<http::Response<Body>, JsValue>> {
        Pin::new(&mut self.inner)
            .poll(cx)
            .map_ok(|res| convert_response(res.into()))
    }
}

impl http_body::Body for Body {
    type Data = Bytes;
    type Error = JsValue;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let inner = if let Some(ref mut inner) = self.inner {
            inner
        } else {
            return Poll::Ready(None);
        };

        Pin::new(&mut inner.chunk)
            .poll(cx)
            .map(|result| {
                inner.chunk = JsFuture::from(inner.reader.read());
                result.map(|v| {
                    v.unchecked_into::<ReadableStreamState>()
                        .value()
                        .map(|chunk| Bytes::from(chunk.to_vec()))
                })
            })
            .map(Result::transpose)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<Option<http::header::HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

fn convert_request(req: http::Request<&[u8]>) -> Result<web_sys::Request, JsValue> {
    let headers = web_sys::Headers::new()?;
    for (k, v) in req.headers() {
        headers.append(k.as_str(), v.to_str().unwrap())?;
    }

    let mut init = web_sys::RequestInit::new();
    init.method(req.method().as_str()).headers(&headers);
    if !req.body().is_empty() {
        init.body(Some(Uint8Array::from(*req.body()).as_ref()));
    }
    web_sys::Request::new_with_str_and_init(&req.uri().to_string(), &init)
}

fn convert_response(res: web_sys::Response) -> http::Response<Body> {
    let builder = http::Response::builder();

    let mut builder = builder.status(http::StatusCode::from_u16(res.status()).unwrap());

    for result in js_sys::try_iter(res.headers().as_ref()).unwrap().unwrap() {
        let entry = js_sys::Array::from(&result.unwrap());
        let (k, v) = (
            entry.get(0).as_string().unwrap(),
            entry.get(1).as_string().unwrap(),
        );
        let v = http::header::HeaderValue::from_maybe_shared(Bytes::from(v)).unwrap();
        builder = builder.header(&k, v);
    }

    let inner = res.body().map(|body| {
        let reader = ReadableStream::from(body).get_reader();
        BodyInner {
            chunk: JsFuture::from(reader.read()),
            reader,
        }
    });
    builder.body(Body { inner }).unwrap()
}
