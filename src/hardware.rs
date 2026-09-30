use crate::nvml::{GpuReading, Nvml};
use std::{mem::size_of, ptr::null, time::Instant};
use windows_sys::{
    Win32::System::{Performance::*, Registry::*, SystemInformation::*, Threading::*},
    core::w,
};

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

#[derive(Clone, Debug)]
pub struct LogicalCpu {
    pub group: u16,
    pub index: u32,
    pub usage: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryReading {
    pub total: u64,
    pub available: u64,
}
impl MemoryReading {
    pub fn used(self) -> u64 {
        self.total.saturating_sub(self.available)
    }
    pub fn percent(self) -> f64 {
        self.used() as f64 * 100.0 / self.total as f64
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cpu_name: String,
    pub cpu_total: Option<f64>,
    pub cpus: Vec<LogicalCpu>,
    pub cpu_error: Option<String>,
    pub ram: Option<MemoryReading>,
    pub gpus: Vec<GpuReading>,
    pub sampled_at: Option<Instant>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            cpu_name: "Starting...".into(),
            cpu_total: None,
            cpus: Vec::new(),
            cpu_error: None,
            ram: None,
            gpus: Vec::new(),
            sampled_at: None,
        }
    }
}

pub struct Sampler {
    cpu: Result<Cpu, String>,
    gpu: Result<Nvml, String>,
    cpu_name: String,
}
impl Sampler {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            gpu: Nvml::new(),
            cpu_name: cpu_name(),
        }
    }
    pub fn sample(&mut self) -> Snapshot {
        let (cpu_total, cpus, cpu_error) = match &mut self.cpu {
            Ok(cpu) => cpu.sample(),
            Err(error) => (None, Vec::new(), Some(error.clone())),
        };
        let gpus = match &self.gpu {
            Ok(gpu) => gpu.sample(),
            Err(error) => vec![GpuReading::unavailable(error.clone())],
        };
        Snapshot {
            cpu_name: self.cpu_name.clone(),
            cpu_total,
            cpus,
            cpu_error,
            ram: memory(),
            gpus,
            sampled_at: Some(Instant::now()),
        }
    }
}

fn cpu_name() -> String {
    let mut buffer = [0u16; 256];
    let mut bytes = size_of::<[u16; 256]>() as u32;
    // SAFETY: the registry API writes at most `bytes` into this live UTF-16 buffer.
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0"),
            w!("ProcessorNameString"),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if result != 0 {
        return "CPU".into();
    }
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end]).trim().to_string()
}

fn memory() -> Option<MemoryReading> {
    let mut value = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: initialized structure has the required size and a writable lifetime.
    if unsafe { GlobalMemoryStatusEx(&mut value) } == 0 || value.ullTotalPhys == 0 {
        return None;
    }
    Some(MemoryReading {
        total: value.ullTotalPhys,
        available: value.ullAvailPhys,
    })
}

struct Counter {
    handle: PDH_HCOUNTER,
    group: u16,
    index: u32,
}
struct Cpu {
    query: PDH_HQUERY,
    total: PDH_HCOUNTER,
    counters: Vec<Counter>,
    primed: bool,
    last_collect: Instant,
}
impl Cpu {
    fn new() -> Result<Self, String> {
        let mut query = std::ptr::null_mut();
        // SAFETY: local query output is valid; no log source is requested.
        let status = unsafe { PdhOpenQueryW(null(), 0, &mut query) };
        if status != 0 {
            return Err(format!("PDH open: 0x{status:08X}"));
        }
        let mut cpu = Self {
            query,
            total: std::ptr::null_mut(),
            counters: Vec::new(),
            primed: false,
            last_collect: Instant::now(),
        };
        cpu.total = cpu.add("\\Processor Information(_Total)\\% Processor Time")?;
        // SAFETY: topology queries have no pointer arguments. Enumerate all processor groups.
        let groups = unsafe { GetActiveProcessorGroupCount() };
        for group in 0..groups {
            let count = unsafe { GetActiveProcessorCount(group) };
            for index in 0..count {
                let path = format!("\\Processor Information({group},{index})\\% Processor Time");
                cpu.counters.push(Counter {
                    handle: cpu.add(&path)?,
                    group,
                    index,
                });
            }
        }
        cpu.primed = unsafe { PdhCollectQueryData(query) } == 0;
        Ok(cpu)
    }
    fn add(&self, path: &str) -> Result<PDH_HCOUNTER, String> {
        let path = wide(path);
        let mut handle = std::ptr::null_mut();
        // SAFETY: query is live, path is terminated and output points to a local handle.
        let status = unsafe { PdhAddEnglishCounterW(self.query, path.as_ptr(), 0, &mut handle) };
        if status != 0 {
            return Err(format!("PDH counter: 0x{status:08X}"));
        }
        Ok(handle)
    }
    fn sample(&mut self) -> (Option<f64>, Vec<LogicalCpu>, Option<String>) {
        // Rate counters need an interval; do not interpret the immediate startup sample as 0%.
        if self.last_collect.elapsed() < std::time::Duration::from_millis(100) {
            let cpus = self
                .counters
                .iter()
                .map(|c| LogicalCpu {
                    group: c.group,
                    index: c.index,
                    usage: None,
                })
                .collect();
            return (None, cpus, Some("CPU warming up".into()));
        }
        // SAFETY: query and all counters remain owned by this Cpu on this thread.
        let status = unsafe { PdhCollectQueryData(self.query) };
        let ready = self.primed && status == 0;
        self.primed = status == 0;
        self.last_collect = Instant::now();
        let total = if ready {
            counter_value(self.total)
        } else {
            None
        };
        let cpus: Vec<LogicalCpu> = self
            .counters
            .iter()
            .map(|c| LogicalCpu {
                group: c.group,
                index: c.index,
                usage: if ready { counter_value(c.handle) } else { None },
            })
            .collect();
        let error = if status != 0 {
            Some(format!("PDH sample: 0x{status:08X}"))
        } else if !ready {
            Some("CPU warming up".into())
        } else if cpus.iter().any(|cpu| cpu.usage.is_none()) {
            Some("Logical CPU counter unavailable".into())
        } else if total.is_none() {
            Some("CPU counter unavailable".into())
        } else {
            None
        };
        (total, cpus, error)
    }
}
impl Drop for Cpu {
    fn drop(&mut self) {
        // SAFETY: this query is owned once; closing it also releases its counters.
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}
fn counter_value(handle: PDH_HCOUNTER) -> Option<f64> {
    let mut value = PDH_FMT_COUNTERVALUE::default();
    // SAFETY: the live handle fills this correctly sized output structure.
    let status = unsafe {
        PdhGetFormattedCounterValue(handle, PDH_FMT_DOUBLE, std::ptr::null_mut(), &mut value)
    };
    if status != 0 || !matches!(value.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA) {
        return None;
    }
    // SAFETY: PDH_FMT_DOUBLE above determines the active union field.
    let value = unsafe { value.Anonymous.doubleValue };
    value.is_finite().then_some(value.clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_used_saturates_if_available_exceeds_total() {
        assert_eq!(
            MemoryReading {
                total: 8,
                available: 12
            }
            .used(),
            0
        );
        assert_eq!(
            MemoryReading {
                total: 8,
                available: 2
            }
            .percent(),
            75.0
        );
    }
}
