// Измерение и освобождение памяти процесса.
// Windows: рабочий набор (то, что показывает диспетчер задач) и EmptyWorkingSet.
// Linux: RSS и malloc_trim.

#[cfg(windows)]
mod sys {
    #[repr(C)]
    struct Pmc { cb: u32, page_faults: u32, peak_ws: usize, ws: usize, a: usize, b: usize, c: usize, d: usize, pagefile: usize, peak_pagefile: usize }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(h: isize, p: *mut Pmc, cb: u32) -> i32;
        fn K32EmptyWorkingSet(h: isize) -> i32;
    }
    pub fn rss_bytes() -> u64 {
        unsafe {
            let mut m = Pmc { cb: std::mem::size_of::<Pmc>() as u32, page_faults: 0, peak_ws: 0, ws: 0, a: 0, b: 0, c: 0, d: 0, pagefile: 0, peak_pagefile: 0 };
            if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) != 0 { m.ws as u64 } else { 0 }
        }
    }
    pub fn trim() { unsafe { let _ = K32EmptyWorkingSet(GetCurrentProcess()); } }
}

#[cfg(target_os = "linux")]
mod sys {
    extern "C" { fn malloc_trim(pad: usize) -> i32; }
    pub fn rss_bytes() -> u64 {
        std::fs::read_to_string("/proc/self/statm").ok().and_then(|s| s.split_whitespace().nth(1).and_then(|x| x.parse::<u64>().ok())).map(|p| p * 4096).unwrap_or(0)
    }
    pub fn trim() { unsafe { let _ = malloc_trim(0); } }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod sys {
    pub fn rss_bytes() -> u64 { 0 }
    pub fn trim() {}
}

pub fn rss_mb() -> f64 { sys::rss_bytes() as f64 / 1_048_576.0 }
pub fn trim() { sys::trim(); }

/// Мгновенное завершение без запуска деструкторов: у TDLib остаются живые потоки,
/// обычный выход из процесса может закончиться аварией. Вызывать после корректного закрытия клиента.
pub fn fast_exit(code: i32) -> ! {
    #[cfg(windows)]
    unsafe {
        extern "system" { fn TerminateProcess(h: isize, code: u32) -> i32; fn GetCurrentProcess() -> isize; }
        let _ = TerminateProcess(GetCurrentProcess(), code as u32);
    }
    #[cfg(unix)]
    unsafe {
        extern "C" { fn _exit(code: i32) -> !; }
        _exit(code);
    }
    #[allow(unreachable_code)]
    std::process::exit(code)
}
