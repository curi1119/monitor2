use crate::hardware::LogicalCpu;
use std::{
    mem::{offset_of, size_of},
    ptr,
};
use windows_sys::Win32::System::SystemInformation::*;

pub struct Topology {
    cores: Vec<Vec<(u16, u32)>>,
}
impl Topology {
    pub fn new() -> Self {
        let mut bytes = 0;
        // SAFETY: first call requests the size, second uses an 8-byte aligned owned buffer.
        unsafe {
            GetLogicalProcessorInformationEx(RelationProcessorCore, ptr::null_mut(), &mut bytes);
        }
        if bytes == 0 {
            return Self { cores: Vec::new() };
        }
        let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
        if unsafe {
            GetLogicalProcessorInformationEx(
                RelationProcessorCore,
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        } == 0
        {
            return Self { cores: Vec::new() };
        }
        let mut cores = Vec::new();
        let mut offset = 0usize;
        let base = buffer.as_ptr().cast::<u8>();
        while offset + 8 <= bytes as usize {
            // SAFETY: validated record boundary; only read the fixed header before inspecting its size.
            let header = unsafe { ptr::read_unaligned(base.add(offset).cast::<[u32; 2]>()) };
            let length = header[1] as usize;
            if length < 8 || offset + length > bytes as usize {
                break;
            }
            if header[0] == RelationProcessorCore as u32
                && length >= 8 + size_of::<PROCESSOR_RELATIONSHIP>()
            {
                let processor = unsafe {
                    ptr::read_unaligned(base.add(offset + 8).cast::<PROCESSOR_RELATIONSHIP>())
                };
                let group_offset = 8 + offset_of!(PROCESSOR_RELATIONSHIP, GroupMask);
                if group_offset + processor.GroupCount as usize * size_of::<GROUP_AFFINITY>()
                    <= length
                {
                    let mut members = Vec::new();
                    for group_index in 0..processor.GroupCount as usize {
                        let affinity = unsafe {
                            ptr::read_unaligned(
                                base.add(
                                    offset
                                        + group_offset
                                        + group_index * size_of::<GROUP_AFFINITY>(),
                                )
                                .cast::<GROUP_AFFINITY>(),
                            )
                        };
                        for bit in 0..usize::BITS {
                            if affinity.Mask & (1usize << bit) != 0 {
                                members.push((affinity.Group, bit));
                            }
                        }
                    }
                    if !members.is_empty() {
                        cores.push(members);
                    }
                }
            }
            offset += length;
        }
        Self { cores }
    }
    pub fn physical(&self, logical: &[LogicalCpu]) -> Vec<LogicalCpu> {
        self.cores
            .iter()
            .enumerate()
            .map(|(index, members)| {
                let readings: Option<Vec<f64>> = members
                    .iter()
                    .map(|&(g, i)| {
                        logical
                            .iter()
                            .find(|c| c.group == g && c.index == i)
                            .and_then(|c| c.usage)
                    })
                    .collect();
                LogicalCpu {
                    group: 0,
                    index: index as u32,
                    usage: readings.map(|r| r.iter().sum::<f64>() / r.len() as f64),
                }
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smt_siblings_are_averaged_not_dropped() {
        let t = Topology {
            cores: vec![vec![(0, 0), (0, 1)], vec![(1, 0), (1, 1)]],
        };
        let cpus = vec![
            LogicalCpu {
                group: 0,
                index: 0,
                usage: Some(100.0),
            },
            LogicalCpu {
                group: 0,
                index: 1,
                usage: Some(0.0),
            },
            LogicalCpu {
                group: 1,
                index: 0,
                usage: Some(20.0),
            },
            LogicalCpu {
                group: 1,
                index: 1,
                usage: None,
            },
        ];
        let physical = t.physical(&cpus);
        assert_eq!(physical[0].usage, Some(50.0));
        assert_eq!(physical[1].usage, None);
    }
}
