//! OS scheduling hints so the accuracy pass never slows live detection. On a 4-core laptop a
//! normal-priority accuracy pass made live decodes ~5x slower (327 → 1,872 ms median); one
//! low-priority thread beside a high-priority live thread costs ~130 ms instead.
//! whisper.cpp's worker threads are created by the calling thread and inherit its class.

/// Live audio and transcription: latency-critical.
pub fn interactive() {
    #[cfg(target_os = "macos")]
    // SAFETY: only changes the calling thread's QoS class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0);
    }
}

/// The accuracy pass: useful but never urgent. On Apple Silicon this also steers it to the
/// efficiency cores.
pub fn background() {
    #[cfg(target_os = "macos")]
    // SAFETY: only changes the calling thread's QoS class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
    #[cfg(target_os = "linux")]
    // SAFETY: renices only the calling thread (threads it creates inherit the nice value).
    unsafe {
        let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
        libc::setpriority(libc::PRIO_PROCESS, tid, 10);
    }
}

/// Threads for the accuracy pass. On GPU the chip does the work; on CPU it gets what the live
/// decoder leaves of the physical cores, and at least one.
pub fn accuracy_threads(use_gpu: bool, live_threads: usize) -> usize {
    if use_gpu {
        return live_threads;
    }
    let logical = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let physical = sysinfo::System::new().physical_core_count().unwrap_or(logical / 2).max(1);
    physical.saturating_sub(live_threads).max(1)
}

