use crate::backend::webhid::utils;
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use wasm_bindgen::JsValue;

#[derive(Debug, Clone, PartialEq)]
pub struct HashableJsValue(JsValue);

impl From<JsValue> for HashableJsValue {
    fn from(value: JsValue) -> Self {
        HashableJsValue(value)
    }
}

impl Into<JsValue> for HashableJsValue {
    fn into(self) -> JsValue {
        self.0
    }
}

impl Display for HashableJsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        utils::to_string(&self.0).fmt(f)
    }
}

impl Hash for HashableJsValue {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        utils::to_string(&self.0).hash(hasher)
    }
}

impl Eq for HashableJsValue {}

impl Deref for HashableJsValue {
    type Target = JsValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for HashableJsValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
