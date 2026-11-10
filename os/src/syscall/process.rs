//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// TODO: implement the syscall
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");

    // 先准备好返回值，默认是 -1，表示失败
    let mut result: isize = -1;

    if trace_request == 0 {
        // 0 表示读：读地址 id 处的一个字节
        let ptr = id as *const u8;
        let value = unsafe { *ptr };
        result = value as isize;
    } else if trace_request == 1 {
        // 1 表示写：把 data 的最低 8 位写到地址 id 处
        let ptr = id as *mut u8;
        unsafe {
            *ptr = data as u8;
        }
        result = 0;
    } else if trace_request == 2 {
        // 2 表示查询：当前任务对系统调用 id 的调用次数
        let count = crate::task::get_syscall_count(id);
        result = count as isize;
    }
    result
}
