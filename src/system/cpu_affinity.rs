/// 跨平台设置 CPU 亲和性模块

/// 为当前线程设置 CPU 核心绑定
/// 参数 core_id: 核心索引（从 0 开始）
/// 返回值: 成功返回 true，失败返回 false
pub fn set_core(core_id: usize) -> bool {
    #[cfg(target_os = "linux")]
    {
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_SET(core_id, &mut set);
            let tid = libc::pthread_self();
            libc::pthread_setaffinity_np(tid, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadAffinityMask};
        unsafe {
            // 注意：1 << core_id 仅适用于前 64 个核心
            let mask = 1 << core_id;
            let handle = GetCurrentThread();
            SetThreadAffinityMask(handle, mask) != 0
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::mem;
        #[repr(C)]
        struct ThreadAffinityPolicy { affinity_tag: i32 }
        const THREAD_AFFINITY_POLICY: i32 = 4;
        
        unsafe {
            let port = libc::mach_thread_self();
            let mut policy = ThreadAffinityPolicy { affinity_tag: core_id as i32 };
            let result = libc::thread_policy_set(
                port,
                THREAD_AFFINITY_POLICY as u32,
                &mut policy as *mut _ as *mut i32,
                (mem::size_of::<ThreadAffinityPolicy>() / mem::size_of::<i32>()) as u32,
            );
            result == 0
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = core_id;
        false
    }
}