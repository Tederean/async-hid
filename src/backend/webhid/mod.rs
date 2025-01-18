use crate::{DeviceCriteria, DeviceInfo, DeviceReader, DeviceWriter, ErrorSource, HidError, HidResult, ensure};
use async_channel::{Receiver, unbounded};
use futures_core::Stream;
use js_sys::wasm_bindgen::JsValue;
use pollster::block_on;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{HidCollectionInfo, HidDevice, HidDeviceRequestOptions, HidInputReportEvent};

mod hashable_js_value;
mod utils;

pub use self::hashable_js_value::HashableJsValue;

#[derive(Serialize, Deserialize, Debug)]
struct HidDeviceRequest {
    #[serde(rename = "vendorId")]
    pub vendor_id: Option<u16>,
    #[serde(rename = "productId")]
    pub product_id: Option<u16>,
    #[serde(rename = "usagePage")]
    pub usage_page: Option<u16>,
    #[serde(rename = "usage")]
    pub usage_id: Option<u16>,
}

pub async fn enumerate() -> HidResult<impl Stream<Item = DeviceInfo> + Unpin> {
    let api = utils::get_web_hid_api()?;

    let mut found_devices = {
        let hid_device_request = Vec::<HidDeviceRequest>::with_capacity(0);

        let js_options =
            serde_wasm_bindgen::to_value(&hid_device_request).map_err(|x| HidError::custom(format!("Failed to create device filter: {0}.", x)))?;

        let options = HidDeviceRequestOptions::new(&js_options);

        let js_devices = utils::promise_to_future(api.request_device(&options)).await?;

        utils::cast::<js_sys::Array>(&js_devices)?
            .iter()
            .filter_map(|x| match get_device_info(x) {
                Ok(x) => Some(x),
                Err(_) => None,
            })
            .collect::<Vec<_>>()
    };

    let existing_devices = {
        let js_devices = utils::promise_to_future(api.get_devices()).await?;

        utils::cast::<js_sys::Array>(&js_devices)?
            .iter()
            .filter_map(|x| match get_device_info(x) {
                Ok(x) => Some(x),
                Err(_) => None,
            })
            .collect::<Vec<_>>()
    };

    for item in existing_devices {
        if !found_devices.contains(&item) {
            found_devices.push(item);
        }
    }

    Ok(utils::iter(found_devices))
}

pub async fn enumerate_with_criteria(device_criteria: Vec<DeviceCriteria>) -> HidResult<impl Stream<Item = DeviceInfo> + Unpin> {
    let api = utils::get_web_hid_api()?;

    let hid_device_request = device_criteria
        .into_iter()
        .map(|x| HidDeviceRequest {
            vendor_id: x.vendor_id,
            product_id: x.product_id,
            usage_page: x.usage_page,
            usage_id: x.usage_id,
        })
        .collect::<Vec<HidDeviceRequest>>();

    let js_options =
        serde_wasm_bindgen::to_value(&hid_device_request).map_err(|x| HidError::custom(format!("Failed to create device filter: {0}.", x)))?;

    let options = HidDeviceRequestOptions::new(&js_options);

    let js_devices = utils::promise_to_future(api.request_device(&options)).await?;

    let devices = utils::cast::<js_sys::Array>(&js_devices)?
        .iter()
        .filter_map(|x| match get_device_info(x) {
            Ok(x) => Some(x),
            Err(_) => None,
        })
        .collect::<Vec<_>>();

    Ok(utils::iter(devices))
}

fn get_device_info(js_hid_device: JsValue) -> HidResult<DeviceInfo> {
    utils::is_valid_object(&js_hid_device)?;

    let device = utils::cast::<HidDevice>(&js_hid_device)?;

    let name = device.product_name();
    let product_id = device.product_id();
    let vendor_id = device.vendor_id();

    let js_collections = &device.collections();
    let collections = utils::cast::<js_sys::Array>(&js_collections)?;

    let js_collection = collections
        .iter()
        .next()
        .ok_or(HidError::custom("Invalid device descriptor, collections are empty."))?;
    let collection = utils::cast::<HidCollectionInfo>(&js_collection)?;

    let usage_page = collection
        .get_usage_page()
        .ok_or(HidError::custom("Invalid device descriptor, usage page unavailable."))?;
    let usage_id = collection
        .get_usage()
        .ok_or(HidError::custom("Invalid device descriptor, usage id unavailable."))?;

    Ok(DeviceInfo {
        name,
        product_id,
        vendor_id,
        usage_id,
        usage_page,
        device_object: js_hid_device.into(),
    })
}

pub async fn open_readonly(device_info: &DeviceInfo) -> HidResult<DeviceReader> {
    utils::is_valid_object(&device_info.device_object)?;

    let hid_device = utils::cast::<HidDevice>(&device_info.device_object)?;

    if hid_device.opened() {
        utils::promise_to_future(hid_device.close().into()).await?;
    }

    let input_channel = setup_read_closure(&hid_device);

    utils::promise_to_future(hid_device.open().into()).await?;

    let backend_device = Arc::new(BackendDevice {
        js_hid_device: device_info.device_object.deref().clone(),
    });

    let reader = DeviceReader {
        inner: BackendDeviceReader {
            backend_device,
            input_channel,
        },
        device_info: device_info.clone(),
    };

    Ok(reader)
}

pub async fn open(device_info: &DeviceInfo) -> HidResult<(DeviceReader, DeviceWriter)> {
    utils::is_valid_object(&device_info.device_object)?;

    let hid_device = utils::cast::<HidDevice>(&device_info.device_object)?;

    if hid_device.opened() {
        utils::promise_to_future(hid_device.close().into()).await?;
    }

    let input_channel = setup_read_closure(&hid_device);

    utils::promise_to_future(hid_device.open().into()).await?;

    let backend_device = Arc::new(BackendDevice {
        js_hid_device: device_info.device_object.deref().clone(),
    });

    let reader = DeviceReader {
        inner: BackendDeviceReader {
            backend_device: backend_device.clone(),
            input_channel,
        },
        device_info: device_info.clone(),
    };

    let writer = DeviceWriter {
        inner: BackendDeviceWriter { backend_device },
        device_info: device_info.clone(),
    };

    Ok((reader, writer))
}

#[derive(Debug)]
struct BackendDevice {
    js_hid_device: JsValue,
}

impl Drop for BackendDevice {
    fn drop(&mut self) {
        let js_hid_device = self.js_hid_device.clone();

        block_on(async move {
            let hid_device = match utils::cast::<HidDevice>(&js_hid_device) {
                Ok(x) => x,
                Err(_) => return,
            };

            _ = utils::promise_to_future(hid_device.close()).await
        })
    }
}

#[derive(Debug)]
pub struct BackendDeviceReader {
    backend_device: Arc<BackendDevice>,
    input_channel: Receiver<HidInputReportEvent>,
}

impl Drop for BackendDeviceReader {
    fn drop(&mut self) {
        match utils::cast::<HidDevice>(&self.backend_device.js_hid_device) {
            Ok(x) => x.set_oninputreport(None),
            Err(_) => {}
        }
    }
}

impl BackendDeviceReader {
    pub async fn read_input_report(&self, buf: &mut [u8]) -> HidResult<usize> {
        ensure!(!buf.is_empty(), HidError::zero_sized_data());

        match self.input_channel.recv().await {
            Err(_) => Err(HidError::custom("Input channel closed.")),
            Ok(e) => {
                let data_view = e.data();

                buf[0] = e.report_id();

                let report_count = data_view.byte_length();
                let report_offset = data_view.byte_offset();

                if report_count == 0 {
                    return Ok(1);
                }

                let report_buffer = &mut buf[1..];

                if report_count > report_buffer.len() {
                    return Err(HidError::custom("HID input buffer overflow."));
                }

                for (buffer, index) in report_buffer[..report_count]
                    .iter_mut()
                    .zip(0..report_count)
                {
                    *buffer = data_view.get_uint8(index + report_offset);
                }

                Ok(1 + report_count)
            }
        }
    }
}

#[derive(Debug)]
pub struct BackendDeviceWriter {
    backend_device: Arc<BackendDevice>,
}

unsafe impl Send for BackendDeviceWriter {}
unsafe impl Sync for BackendDeviceWriter {}

impl BackendDeviceWriter {
    pub async fn write_output_report(&self, buf: &[u8]) -> HidResult<()> {
        ensure!(!buf.is_empty(), HidError::zero_sized_data());

        let hid_device = utils::cast::<HidDevice>(&self.backend_device.js_hid_device)?;

        let js_promise = hid_device
            .send_report_with_u8_slice(buf[0], &mut Vec::from(&buf[1..]))
            .map_err(|x| HidError::custom(utils::to_string(&x)))?;

        utils::promise_to_future(js_promise).await?;

        Ok(())
    }
}

#[inline]
fn setup_read_closure(hid_device: &HidDevice) -> Receiver<HidInputReportEvent> {
    let (tx, rx) = unbounded::<HidInputReportEvent>();

    let closure = Closure::wrap(Box::new(move |e: HidInputReportEvent| {
        _ = block_on(tx.send(e));
    }) as Box<dyn FnMut(HidInputReportEvent)>);

    hid_device.set_oninputreport(Some(closure.as_ref().unchecked_ref()));

    closure.forget();

    rx
}

pub type BackendError = String;

impl From<BackendError> for ErrorSource {
    fn from(value: BackendError) -> Self {
        ErrorSource::PlatformSpecific(value)
    }
}
