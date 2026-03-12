//! Version and executable timestamp checks.
//!
//! Mirrors `verchk.cpp/.h` from WWLib.

#[cfg(target_os = "windows")]
use std::ffi::CString;
#[cfg(target_os = "windows")]
use std::io::{Read, Seek, SeekFrom};
#[cfg(target_os = "windows")]
use std::mem::{size_of, MaybeUninit};
#[cfg(target_os = "windows")]
use std::os::windows::io::AsRawHandle;
#[cfg(target_os = "windows")]
use std::ptr;

#[cfg(target_os = "windows")]
use windows::core::PCSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{FILETIME, HANDLE, HINSTANCE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::GetFileTime;
#[cfg(target_os = "windows")]
use windows::Win32::System::SystemServices::{IMAGE_DOS_HEADER, IMAGE_FILE_HEADER};
#[cfg(target_os = "windows")]
use windows::Win32::System::Version::{
    GetFileVersionInfoA, GetFileVersionInfoSizeA, VerQueryValueA, VS_FIXEDFILEINFO,
};

/// Obtain version information from the specified file.
#[cfg(target_os = "windows")]
pub fn get_version_info(filename: &str, file_info: &mut VS_FIXEDFILEINFO) -> bool {
    if filename.is_empty() {
        return false;
    }

    let c_filename = match CString::new(filename) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let mut ver_handle: u32 = 0;
    let ver_info_size =
        unsafe { GetFileVersionInfoSizeA(PCSTR(c_filename.as_ptr() as _), &mut ver_handle) };
    if ver_info_size == 0 {
        return false;
    }

    let mut buffer = vec![0u8; ver_info_size as usize];
    let success = unsafe {
        GetFileVersionInfoA(
            PCSTR(c_filename.as_ptr() as _),
            ver_handle,
            ver_info_size,
            buffer.as_mut_ptr() as _,
        )
    };
    if !success.as_bool() {
        return false;
    }

    let sub_block = CString::new("\\").unwrap();
    let mut data_ptr: *mut core::ffi::c_void = ptr::null_mut();
    let mut data_size: u32 = 0;
    let success = unsafe {
        VerQueryValueA(
            buffer.as_ptr() as _,
            PCSTR(sub_block.as_ptr() as _),
            &mut data_ptr,
            &mut data_size,
        )
    };
    if !success.as_bool() || data_size as usize != size_of::<VS_FIXEDFILEINFO>() {
        return false;
    }

    unsafe {
        *file_info = ptr::read(data_ptr as *const VS_FIXEDFILEINFO);
    }

    true
}

/// Retrieve creation time of specified file.
#[cfg(target_os = "windows")]
pub fn get_file_creation_time(filename: &str, create_time: &mut FILETIME) -> bool {
    create_time.dwLowDateTime = 0;
    create_time.dwHighDateTime = 0;

    let file = match std::fs::File::open(filename) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let handle = HANDLE(file.as_raw_handle() as isize);
    let success = unsafe { GetFileTime(handle, ptr::null_mut(), ptr::null_mut(), create_time) };
    success.as_bool()
}

/// Read the image header from a file on disk.
#[cfg(target_os = "windows")]
pub fn get_image_file_header_from_file(
    filename: &str,
    file_header: &mut IMAGE_FILE_HEADER,
) -> bool {
    let mut file = match std::fs::File::open(filename) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let mut dos_header: IMAGE_DOS_HEADER = unsafe { MaybeUninit::zeroed().assume_init() };
    let dos_slice = unsafe {
        std::slice::from_raw_parts_mut(
            &mut dos_header as *mut IMAGE_DOS_HEADER as *mut u8,
            size_of::<IMAGE_DOS_HEADER>(),
        )
    };
    if file.read_exact(dos_slice).is_err() {
        return false;
    }

    let file_header_offset = dos_header.e_lfanew as u64 + size_of::<u32>() as u64;
    if file.seek(SeekFrom::Start(file_header_offset)).is_err() {
        return false;
    }

    let header_slice = unsafe {
        std::slice::from_raw_parts_mut(
            file_header as *mut IMAGE_FILE_HEADER as *mut u8,
            size_of::<IMAGE_FILE_HEADER>(),
        )
    };
    file.read_exact(header_slice).is_ok()
}

/// Read the image header from a loaded module.
#[cfg(target_os = "windows")]
pub fn get_image_file_header_from_instance(
    app_instance: HINSTANCE,
    file_header: &mut IMAGE_FILE_HEADER,
) -> bool {
    if app_instance.0 == 0 {
        return false;
    }

    unsafe {
        let dos_header = app_instance.0 as *const IMAGE_DOS_HEADER;
        if dos_header.is_null() {
            return false;
        }
        let image_header_offset = (*dos_header).e_lfanew as isize + size_of::<u32>() as isize;
        let header_ptr =
            (dos_header as *const u8).offset(image_header_offset) as *const IMAGE_FILE_HEADER;
        *file_header = ptr::read_unaligned(header_ptr);
    }

    true
}

/// Compare executable timestamp against a file on disk.
#[cfg(target_os = "windows")]
pub fn compare_exe_version(app_instance: isize, filename: &str) -> i32 {
    let mut header1: IMAGE_FILE_HEADER = unsafe { MaybeUninit::zeroed().assume_init() };
    let mut header2: IMAGE_FILE_HEADER = unsafe { MaybeUninit::zeroed().assume_init() };
    let got_headers = get_image_file_header_from_instance(HINSTANCE(app_instance), &mut header1)
        && get_image_file_header_from_file(filename, &mut header2);
    if !got_headers {
        return 0;
    }

    let diff = header1.TimeDateStamp as i64 - header2.TimeDateStamp as i64;
    if diff > i32::MAX as i64 {
        i32::MAX
    } else if diff < i32::MIN as i64 {
        i32::MIN
    } else {
        diff as i32
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_version_info(_filename: &str, _file_info: &mut ()) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn get_file_creation_time(_filename: &str, _create_time: &mut ()) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn get_image_file_header_from_file(_filename: &str, _file_header: &mut ()) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn get_image_file_header_from_instance(_app_instance: (), _file_header: &mut ()) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn compare_exe_version(_app_instance: isize, _filename: &str) -> i32 {
    0
}
