//! FFI 缓冲区：Rust 分配的字节序列，由 `gv_buffer_free` 释放并清零。
//! 使用 Box 字节切片（len == capacity），释放时按精确长度重建，不猜测 capacity。

use std::ptr;

#[repr(C)]
pub struct GvBuffer {
    pub data: *mut u8,
    pub len: usize,
}

impl GvBuffer {
    /// 已清零、不持有任何分配的 out buffer 初始形态。
    pub const fn empty() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
        }
    }
}

/// 分配缓冲区：字节切片装箱后移交指针。无 NUL 终止要求。
pub fn allocate(bytes: Vec<u8>) -> GvBuffer {
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    let data = Box::into_raw(boxed) as *mut u8;
    GvBuffer { data, len }
}

/// 释放 `gv_*` 调用返回的缓冲区并把调用方的结构体清零。
///
/// # Safety
/// `buffer` 必须为 null，或指向本库某次成功填充、且尚未释放的 `GvBuffer`。
/// 同一缓冲区只能释放一次；释放后再使用是未定义行为。
pub unsafe fn free(buffer: *mut GvBuffer) {
    if buffer.is_null() {
        return;
    }
    let buffer = &mut *buffer;
    if buffer.data.is_null() {
        buffer.len = 0;
        return;
    }
    let slice = std::slice::from_raw_parts_mut(buffer.data, buffer.len);
    drop(Box::from_raw(slice as *mut [u8]));
    buffer.data = ptr::null_mut();
    buffer.len = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_then_free_round_trips_bytes() {
        let payload: Vec<u8> = "GitaView 原生契约".as_bytes().to_vec();
        let mut buffer = allocate(payload.clone());
        assert_eq!(buffer.len, payload.len());
        let view = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) };
        assert_eq!(view, payload.as_slice());
        unsafe { free(&mut buffer) };
        assert!(buffer.data.is_null());
        assert_eq!(buffer.len, 0);
    }

    #[test]
    fn free_handles_null_pointer_and_empty_buffer_without_allocations() {
        unsafe { free(ptr::null_mut()) };
        let mut empty = GvBuffer::empty();
        unsafe { free(&mut empty) };
        assert!(empty.data.is_null());
    }
}
