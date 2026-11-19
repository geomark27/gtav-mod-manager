//! PE version resource reader (Windows only). The only module of the crate with `unsafe`.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
};

const FIXED_FILE_INFO_SIGNATURE: u32 = 0xFEEF_04BD;

/// Returns `"a.b.c.d"` from the fixed file info of the version resource.
pub(super) fn file_version(path: &Path) -> Option<String> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut handle: u32 = 0;
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 string that outlives the call and
    // `handle` is a valid writable u32.
    let size = unsafe { GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    // SAFETY: `wide` is NUL-terminated; `buf` is writable for exactly `size` bytes.
    let ok = unsafe { GetFileVersionInfoW(wide.as_ptr(), 0, size, buf.as_mut_ptr().cast()) };
    if ok == 0 {
        return None;
    }
    let root: Vec<u16> = "\\".encode_utf16().chain(std::iter::once(0)).collect();
    let mut ptr: *mut c_void = std::ptr::null_mut();
    let mut len: u32 = 0;
    // SAFETY: `buf` holds a version block filled by GetFileVersionInfoW; `root` is a
    // NUL-terminated UTF-16 string; `ptr` and `len` are valid writable outputs. On success
    // `ptr` points inside `buf`, which stays alive until the end of this function.
    let ok = unsafe { VerQueryValueW(buf.as_ptr().cast(), root.as_ptr(), &mut ptr, &mut len) };
    if ok == 0 || ptr.is_null() || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>() {
        return None;
    }
    // SAFETY: `ptr` is non-null and the block holds at least size_of::<VS_FIXEDFILEINFO>()
    // readable bytes inside `buf`; an unaligned read avoids any alignment assumption.
    let info = unsafe { std::ptr::read_unaligned(ptr.cast::<VS_FIXEDFILEINFO>()) };
    if info.dwSignature != FIXED_FILE_INFO_SIGNATURE {
        return None;
    }
    Some(format!(
        "{}.{}.{}.{}",
        info.dwFileVersionMS >> 16,
        info.dwFileVersionMS & 0xFFFF,
        info.dwFileVersionLS >> 16,
        info.dwFileVersionLS & 0xFFFF
    ))
}
