//! System statistics for the F3 screen: this program's CPU, RAM and GPU usage next to the
//! whole system's. Sampled once a second on a background thread (the GPU counters can take a
//! few milliseconds to read), so the game never waits for them, and only while the F3
//! screen is open.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The latest statistics, and the switch that keeps the sampler running.
pub struct Monitor {
    stats: Arc<Mutex<SysStats>>,
    active: Arc<AtomicBool>,
}

impl Monitor {
    /// Samples only while `on` (the F3 screen is shown).
    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }

    pub fn get(&self) -> SysStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

#[derive(Clone, Default)]
pub struct SysStats {
    /// Processor name.
    pub cpu_name: String,
    /// Logical processors.
    pub threads: usize,
    /// CPU time used by this program, percent of the whole processor (all cores).
    pub cpu_game: f32,
    /// CPU usage of the whole system, percent.
    pub cpu_total: f32,
    /// Memory used by this program (private bytes).
    pub ram_game: u64,
    /// Physical memory in use by the whole system, and installed.
    pub ram_used: u64,
    pub ram_total: u64,
    /// GPU usage (busiest engine, like Task Manager), percent; None if not available.
    pub gpu_game: Option<f32>,
    pub gpu_total: Option<f32>,
}

/// Starts the sampler thread; while active, the stats are updated about once a second.
pub fn start() -> Monitor {
    let stats = Arc::new(Mutex::new(SysStats {
        cpu_name: cpu_name(),
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        ..Default::default()
    }));
    let active = Arc::new(AtomicBool::new(false));
    let (out, on) = (stats.clone(), active.clone());
    let _ = std::thread::Builder::new()
        .name("stats".into())
        .spawn(move || {
            let mut sampler = imp::Sampler::new();
            let mut was_on = false;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let now_on = on.load(Ordering::Relaxed);
                if !now_on {
                    was_on = false;
                    continue;
                }
                let mut s = out.lock().map(|s| s.clone()).unwrap_or_default();
                sampler.sample(&mut s);
                // The first sample after a pause only restarts the usage counters (its
                // averages would span the whole pause).
                if was_on {
                    if let Ok(mut o) = out.lock() {
                        *o = s;
                    }
                }
                was_on = true;
            }
        });
    Monitor { stats, active }
}

/// Processor brand string from CPUID.
fn cpu_name() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::__cpuid;
        if __cpuid(0x8000_0000).eax >= 0x8000_0004 {
            let mut bytes = Vec::with_capacity(48);
            for leaf in 0x8000_0002u32..=0x8000_0004 {
                let r = __cpuid(leaf);
                for v in [r.eax, r.ebx, r.ecx, r.edx] {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
            let s = String::from_utf8_lossy(&bytes);
            return s.trim_matches(char::from(0)).trim().to_string();
        }
    }
    "Unknown CPU".into()
}

#[cfg(windows)]
mod imp {
    use super::SysStats;
    use std::collections::HashMap;

    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCountersEx {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    #[repr(C)]
    struct PdhValue {
        status: u32,
        value: f64,
    }

    #[repr(C)]
    struct PdhItem {
        name: *const u16,
        value: PdhValue,
    }

    const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
    const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
    const PDH_MORE_DATA: u32 = 0x8000_07D2;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn GetCurrentProcessId() -> u32;
        fn K32GetProcessMemoryInfo(p: isize, c: *mut ProcessMemoryCountersEx, cb: u32) -> i32;
        fn GetProcessTimes(
            p: isize,
            create: *mut u64,
            exit: *mut u64,
            kernel: *mut u64,
            user: *mut u64,
        ) -> i32;
        fn GetSystemTimes(idle: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
        fn GlobalMemoryStatusEx(m: *mut MemoryStatusEx) -> i32;
    }

    #[link(name = "pdh")]
    extern "system" {
        fn PdhOpenQueryW(source: *const u16, user: usize, query: *mut isize) -> u32;
        fn PdhAddEnglishCounterW(
            query: isize,
            path: *const u16,
            user: usize,
            counter: *mut isize,
        ) -> u32;
        fn PdhCollectQueryData(query: isize) -> u32;
        fn PdhGetFormattedCounterArrayW(
            counter: isize,
            format: u32,
            size: *mut u32,
            count: *mut u32,
            items: *mut PdhItem,
        ) -> u32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    pub struct Sampler {
        last_process: Option<u64>,
        last_system: Option<(u64, u64)>,
        /// GPU engine utilization counter (all engines of all processes).
        gpu: Option<(isize, isize)>,
        pid_tag: String,
    }

    impl Sampler {
        pub fn new() -> Self {
            // SAFETY: plain Win32 calls with valid out pointers.
            let gpu = unsafe {
                let mut query = 0;
                let mut counter = 0;
                let path = wide(r"\GPU Engine(*)\Utilization Percentage");
                if PdhOpenQueryW(std::ptr::null(), 0, &mut query) == 0
                    && PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) == 0
                {
                    PdhCollectQueryData(query);
                    Some((query, counter))
                } else {
                    None
                }
            };
            Self {
                last_process: None,
                last_system: None,
                gpu,
                pid_tag: format!("pid_{}_", unsafe { GetCurrentProcessId() }),
            }
        }

        pub fn sample(&mut self, s: &mut SysStats) {
            // SAFETY: all Win32 calls get properly sized, writable structures.
            unsafe {
                let me = GetCurrentProcess();
                let mut pmc = ProcessMemoryCountersEx {
                    cb: size_of::<ProcessMemoryCountersEx>() as u32,
                    ..Default::default()
                };
                if K32GetProcessMemoryInfo(me, &mut pmc, pmc.cb) != 0 {
                    s.ram_game = pmc.private_usage as u64;
                }
                let mut ms = MemoryStatusEx {
                    length: size_of::<MemoryStatusEx>() as u32,
                    ..Default::default()
                };
                if GlobalMemoryStatusEx(&mut ms) != 0 {
                    s.ram_total = ms.total_phys;
                    s.ram_used = ms.total_phys - ms.avail_phys;
                }

                // CPU: busy time since the last sample over elapsed time (100 ns units).
                let (mut idle, mut kernel, mut user) = (0u64, 0u64, 0u64);
                let (mut c, mut e, mut pk, mut pu) = (0u64, 0u64, 0u64, 0u64);
                if GetSystemTimes(&mut idle, &mut kernel, &mut user) != 0
                    && GetProcessTimes(me, &mut c, &mut e, &mut pk, &mut pu) != 0
                {
                    // System kernel time includes idle time.
                    let total = kernel + user;
                    let process = pk + pu;
                    if let (Some((lt, li)), Some(lp)) = (self.last_system, self.last_process) {
                        let dt = total.saturating_sub(lt).max(1) as f32;
                        let di = idle.saturating_sub(li) as f32;
                        s.cpu_total = (100.0 * (1.0 - di / dt)).clamp(0.0, 100.0);
                        s.cpu_game =
                            (100.0 * process.saturating_sub(lp) as f32 / dt).clamp(0.0, 100.0);
                    }
                    self.last_system = Some((total, idle));
                    self.last_process = Some(process);
                }
            }
            if let Some((game, total)) = self.gpu_usage() {
                s.gpu_game = Some(game);
                s.gpu_total = Some(total);
            }
        }

        /// (this program, whole system) GPU usage: per engine type the sum over its engines,
        /// then the busiest type, like Task Manager.
        fn gpu_usage(&mut self) -> Option<(f32, f32)> {
            let (query, counter) = self.gpu?;
            // SAFETY: the buffer is sized as PDH asks for, and names point into it.
            unsafe {
                if PdhCollectQueryData(query) != 0 {
                    return None;
                }
                let (mut size, mut count) = (0u32, 0u32);
                let fmt = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;
                let r = PdhGetFormattedCounterArrayW(
                    counter,
                    fmt,
                    &mut size,
                    &mut count,
                    std::ptr::null_mut(),
                );
                if r != PDH_MORE_DATA || size == 0 {
                    return None;
                }
                let n = (size as usize).div_ceil(size_of::<PdhItem>());
                let mut buf: Vec<PdhItem> = Vec::with_capacity(n);
                if PdhGetFormattedCounterArrayW(
                    counter,
                    fmt,
                    &mut size,
                    &mut count,
                    buf.as_mut_ptr(),
                ) != 0
                {
                    return None;
                }
                buf.set_len(count as usize);
                let mut game: HashMap<String, f64> = HashMap::new();
                let mut total: HashMap<String, f64> = HashMap::new();
                for item in &buf {
                    if item.value.status > 1 || item.name.is_null() {
                        continue;
                    }
                    let len = (0..).take_while(|&i| *item.name.add(i) != 0).count();
                    let name = String::from_utf16_lossy(std::slice::from_raw_parts(item.name, len));
                    let Some(kind) = name.rsplit("engtype_").next() else {
                        continue;
                    };
                    *total.entry(kind.to_string()).or_default() += item.value.value;
                    if name.starts_with(&self.pid_tag) {
                        *game.entry(kind.to_string()).or_default() += item.value.value;
                    }
                }
                let max = |m: &HashMap<String, f64>| {
                    m.values().copied().fold(0.0, f64::max).min(100.0) as f32
                };
                Some((max(&game), max(&total)))
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::SysStats;

    pub struct Sampler;

    impl Sampler {
        pub fn new() -> Self {
            Sampler
        }

        pub fn sample(&mut self, _s: &mut SysStats) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads real numbers from the system (`cargo test stats -- --nocapture` prints them).
    #[test]
    fn samples_system_stats() {
        let mut s = SysStats {
            cpu_name: cpu_name(),
            ..Default::default()
        };
        let mut sampler = imp::Sampler::new();
        sampler.sample(&mut s);
        let busy = std::time::Instant::now();
        while busy.elapsed() < Duration::from_millis(600) {
            std::hint::black_box(0u64.wrapping_add(1));
        }
        sampler.sample(&mut s);
        println!(
            "cpu {:?} game {:.1}% total {:.1}% | ram game {} MB, {} / {} MB | gpu {:?} / {:?}",
            s.cpu_name,
            s.cpu_game,
            s.cpu_total,
            s.ram_game >> 20,
            s.ram_used >> 20,
            s.ram_total >> 20,
            s.gpu_game,
            s.gpu_total
        );
        assert!(!s.cpu_name.is_empty());
        if cfg!(windows) {
            assert!(s.ram_game > 0 && s.ram_total > s.ram_used && s.ram_used > 0);
            assert!(s.cpu_game > 0.0 && s.cpu_total >= s.cpu_game * 0.5);
        }
    }
}
