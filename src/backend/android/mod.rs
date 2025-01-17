mod utils;

use std::fmt::{Debug, Formatter};
use android_logger::Config;
use futures_core::Stream;
use log::LevelFilter;
use crate::{DeviceInfo, DeviceReader, DeviceWriter, ErrorSource, HidResult};

pub async fn enumerate() -> HidResult<impl Stream<Item = DeviceInfo> + Unpin + Send> {
    android_logger::init_once(
        Config::default().with_max_level(LevelFilter::Trace),
    );

    let android_context = ndk_context::android_context();

    let vm = unsafe { jni::JavaVM::from_raw(android_context.vm().cast()) }?;

    let _env = vm.attach_current_thread()?;

    let _context = unsafe { jni::objects::JObject::from_raw(android_context.context().cast()) };

    Ok(utils::iter(Vec::<DeviceInfo>::new()))
}

pub async fn open_readonly(_device_info: &DeviceInfo) -> HidResult<DeviceReader> {
    todo!()
}

pub async fn open(_device_info: &DeviceInfo) -> HidResult<(DeviceReader, DeviceWriter)> {
    todo!()
}

#[derive(Debug)]
pub struct BackendDeviceReader {
}

impl BackendDeviceReader {
    pub async fn read_input_report(&self, _buffer: &mut [u8]) -> HidResult<usize> {
        todo!()
    }
}

#[derive(Debug)]
pub struct BackendDeviceWriter {
}

impl BackendDeviceWriter {
    pub async fn write_output_report(&self, _buffer: &[u8]) -> HidResult<()> {
        todo!()
    }
}

pub type BackendError = JvmError;

pub enum JvmError {
    JniError(jni::errors::Error),
    JavaException(jni::errors::Exception)
}

impl Debug for JvmError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<jni::errors::Error> for ErrorSource {
    fn from(value: jni::errors::Error) -> Self {
        ErrorSource::PlatformSpecific(JvmError::JniError(value))
    }
}

impl From<jni::errors::Exception> for ErrorSource {
    fn from(value: jni::errors::Exception) -> Self {
        ErrorSource::PlatformSpecific(JvmError::JavaException(value))
    }
}

impl From<BackendError> for ErrorSource {
    fn from(value: BackendError) -> Self {
        ErrorSource::PlatformSpecific(value)
    }
}
