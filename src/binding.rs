use js_sys::{Promise, Uint8Array};
use wasm_bindgen::{prelude::*, JsCast};

#[wasm_bindgen]
extern "C" {
    pub type ReadableStream;

    #[wasm_bindgen(method, js_name = "getReader")]
    pub fn get_reader(this: &ReadableStream) -> ReadableStreamDefaultReader;

    pub type ReadableStreamDefaultReader;

    #[wasm_bindgen(method)]
    pub fn read(this: &ReadableStreamDefaultReader) -> Promise;

    pub type ReadableStreamState;

    #[wasm_bindgen(method, getter)]
    pub fn value(this: &ReadableStreamState) -> Option<Uint8Array>;
}

impl From<web_sys::ReadableStream> for ReadableStream {
    fn from(s: web_sys::ReadableStream) -> Self {
        s.unchecked_into()
    }
}
