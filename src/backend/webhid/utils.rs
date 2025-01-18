use crate::HidError;
use futures_core::Stream;
use js_sys::Promise;
use js_sys::wasm_bindgen::JsValue;
use js_sys::wasm_bindgen::prelude::wasm_bindgen;
use std::any::type_name;
use std::pin::Pin;
use std::task::{Context, Poll};
use wasm_bindgen_futures::JsFuture;
use web_sys::wasm_bindgen::JsCast;
use web_sys::{Hid, window};

pub fn get_web_hid_api() -> Result<Hid, HidError> {
    let window = window().ok_or(HidError::custom("Failed to get Window object."))?;

    let hid_api = window.navigator().hid();

    match hid_api.is_null() || hid_api.is_undefined() {
        true => Err(HidError::custom("WebHID is not supported by this environment.")),
        false => Ok(hid_api),
    }
}

#[inline]
pub fn cast<T: JsCast>(value: &JsValue) -> Result<&T, HidError> {
    value.dyn_ref::<T>().ok_or(HidError::custom(format!(
        "Failed to cast JavaScript object to type {0}.",
        type_name::<T>()
    )))
}

#[inline]
pub fn is_valid_object(value: &JsValue) -> Result<(), HidError> {
    if value.is_null() {
        return Err(HidError::custom("JavaScript object is null."));
    }

    if value.is_undefined() {
        return Err(HidError::custom("JavaScript object is undefined."));
    }

    Ok(())
}

#[inline]
pub async fn promise_to_future(promise: Promise) -> Result<JsValue, HidError> {
    JsFuture::from(promise)
        .await
        .map_err(|x| HidError::custom(format!("Failed to await JavaScript promise: {0}.", to_string(&x))))
}

pub fn iter<I: IntoIterator>(iter: I) -> Iter<I::IntoIter> {
    Iter { iter: iter.into_iter() }
}

#[derive(Clone, Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Iter<I> {
    iter: I,
}

impl<I> Unpin for Iter<I> {}

impl<I: Iterator> Stream for Iter<I> {
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.iter.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = String)]
    pub fn to_string(value: &JsValue) -> String;
}