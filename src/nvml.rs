use std::{
    ffi::{CStr, c_char, c_void},
    ptr::null_mut,
};
use windows_sys::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
    },
    core::w,
};

// NVML's C ABI uses unsigned ints, unsigned long longs and opaque device pointers.
// The DLL and handles remain on the sampling thread for their entire lifetime.
type Device = *mut c_void;
type Init = unsafe extern "C" fn() -> u32;
type Shutdown = unsafe extern "C" fn() -> u32;
type Count = unsafe extern "C" fn(*mut u32) -> u32;
type Handle = unsafe extern "C" fn(u32, *mut Device) -> u32;
type Name = unsafe extern "C" fn(Device, *mut c_char, u32) -> u32;
type Utilization = unsafe extern "C" fn(Device, *mut UtilizationRaw) -> u32;
type Memory = unsafe extern "C" fn(Device, *mut MemoryRaw) -> u32;
type Temperature = unsafe extern "C" fn(Device, u32, *mut u32) -> u32;
#[repr(C)]
#[derive(Default)]
struct UtilizationRaw {
    gpu: u32,
    memory: u32,
}
#[repr(C)]
#[derive(Default)]
struct MemoryRaw {
    total: u64,
    free: u64,
    used: u64,
}

#[derive(Clone, Debug)]
pub struct GpuReading {
    pub name: String,
    pub usage: Option<f64>,
    pub temperature: Option<u32>,
    pub used: Option<u64>,
    pub total: Option<u64>,
    pub error: Option<String>,
}
impl GpuReading {
    pub fn unavailable(error: String) -> Self {
        Self {
            name: "NVIDIA GPU".into(),
            usage: None,
            temperature: None,
            used: None,
            total: None,
            error: Some(error),
        }
    }
}
struct Library(HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: owned module; all function pointers are discarded before it is released.
        unsafe {
            FreeLibrary(self.0);
        }
    }
}
struct Gpu {
    handle: Device,
    name: String,
    error: Option<String>,
}
pub struct Nvml {
    _library: Library,
    shutdown: Shutdown,
    utilization: Utilization,
    memory: Memory,
    temperature: Temperature,
    devices: Vec<Gpu>,
}
impl Nvml {
    pub fn new() -> Result<Self, String> {
        // SAFETY: a constant terminated name and restricted system-directory search.
        // Never load a nvml.dll supplied in the working directory.
        let module =
            unsafe { LoadLibraryExW(w!("nvml.dll"), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
        if module.is_null() {
            return Err("NVML not installed in System32".into());
        }
        let library = Library(module);
        macro_rules! symbol {
            ($name:literal, $ty:ty) => {{
                // SAFETY: these documented NVML exports have the exact C ABI types above.
                let address = unsafe { GetProcAddress(module, concat!($name, "\0").as_ptr()) }
                    .ok_or_else(|| format!("Missing NVML export: {}", $name))?;
                unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(address) }
            }};
        }
        let init = symbol!("nvmlInit_v2", Init);
        let shutdown = symbol!("nvmlShutdown", Shutdown);
        let count = symbol!("nvmlDeviceGetCount_v2", Count);
        let handle = symbol!("nvmlDeviceGetHandleByIndex_v2", Handle);
        let name = symbol!("nvmlDeviceGetName", Name);
        let utilization = symbol!("nvmlDeviceGetUtilizationRates", Utilization);
        let memory = symbol!("nvmlDeviceGetMemoryInfo", Memory);
        let temperature = symbol!("nvmlDeviceGetTemperature", Temperature);
        // SAFETY: all exports are validated and the library remains owned.
        check(unsafe { init() }, "NVML init")?;
        let mut nvml = Self {
            _library: library,
            shutdown,
            utilization,
            memory,
            temperature,
            devices: Vec::new(),
        };
        let mut device_count = 0;
        check(unsafe { count(&mut device_count) }, "NVML enumerate")?;
        nvml.devices = enumerate_devices(device_count, handle, name);
        if nvml.devices.is_empty() {
            return Err("No NVIDIA GPU detected".into());
        }
        Ok(nvml)
    }
    pub fn sample(&self) -> Vec<GpuReading> {
        self.devices
            .iter()
            .map(|gpu| {
                if let Some(error) = &gpu.error {
                    let mut reading = GpuReading::unavailable(error.clone());
                    reading.name = gpu.name.clone();
                    return reading;
                }
                let mut utilization = UtilizationRaw::default();
                let mut memory = MemoryRaw::default();
                let mut temperature = 0;
                // SAFETY: device handles remain valid until shutdown, buffers match the NVML ABI.
                let u = unsafe { (self.utilization)(gpu.handle, &mut utilization) };
                let m = unsafe { (self.memory)(gpu.handle, &mut memory) };
                let t = unsafe { (self.temperature)(gpu.handle, 0, &mut temperature) };
                let valid_memory = m == 0
                    && memory.total != 0
                    && memory.total != u64::MAX
                    && memory.used != u64::MAX
                    && memory.used <= memory.total;
                let mut errors = Vec::new();
                if u != 0 {
                    errors.push(format!("usage: NVML {u}"));
                }
                if !valid_memory {
                    errors.push(format!("VRAM unavailable (NVML {m})"));
                }
                if t != 0 {
                    errors.push(format!("temperature: NVML {t}"));
                }
                GpuReading {
                    name: gpu.name.clone(),
                    usage: (u == 0 && utilization.gpu <= 100).then_some(utilization.gpu as f64),
                    temperature: (t == 0).then_some(temperature),
                    used: valid_memory.then_some(memory.used),
                    total: valid_memory.then_some(memory.total),
                    error: (!errors.is_empty()).then(|| errors.join(", ")),
                }
            })
            .collect()
    }
}
fn enumerate_devices(count: u32, handle: Handle, name: Name) -> Vec<Gpu> {
    (0..count)
        .map(|index| {
            let mut device = null_mut();
            // SAFETY: caller supplies validated NVML exports; device output and name buffer are live.
            let status = unsafe { handle(index, &mut device) };
            let fallback = format!("NVIDIA GPU {index}");
            if status != 0 || device.is_null() {
                return Gpu {
                    handle: null_mut(),
                    name: fallback,
                    error: Some(if status != 0 {
                        format!("NVML device {index} handle: NVML {status}")
                    } else {
                        format!("NVML device {index} returned a null handle")
                    }),
                };
            }
            let mut buffer = [0 as c_char; 96];
            let status = unsafe { name(device, buffer.as_mut_ptr(), buffer.len() as u32) };
            buffer[95] = 0;
            let gpu_name = if status == 0 {
                // SAFETY: the buffer is terminated even if the DLL response was unexpected.
                unsafe { CStr::from_ptr(buffer.as_ptr()) }
                    .to_string_lossy()
                    .into_owned()
            } else {
                fallback
            };
            Gpu {
                handle: device,
                name: gpu_name,
                error: None,
            }
        })
        .collect()
}

impl Drop for Nvml {
    fn drop(&mut self) {
        // SAFETY: one successful initialization is paired with one shutdown before unloading.
        unsafe {
            (self.shutdown)();
        }
    }
}
fn check(status: u32, operation: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("{operation}: NVML {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" fn mock_handle(index: u32, output: *mut Device) -> u32 {
        if index == 0 {
            return 4;
        } // NVML_ERROR_NO_PERMISSION
        unsafe {
            *output = std::ptr::dangling_mut::<u8>().cast();
        }
        0
    }
    unsafe extern "C" fn mock_name(_: Device, output: *mut c_char, capacity: u32) -> u32 {
        let text = b"Working GPU\0";
        assert!(capacity as usize >= text.len());
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr().cast(), output, text.len());
        }
        0
    }
    #[test]
    fn inaccessible_device_does_not_discard_healthy_devices() {
        let devices = enumerate_devices(2, mock_handle, mock_name);
        assert_eq!(devices.len(), 2);
        assert!(devices[0].error.as_ref().unwrap().contains("NVML 4"));
        assert!(devices[0].handle.is_null());
        assert_eq!(devices[1].name, "Working GPU");
        assert!(devices[1].error.is_none());
        assert!(!devices[1].handle.is_null());
    }
}
