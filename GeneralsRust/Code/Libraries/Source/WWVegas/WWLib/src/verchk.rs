//! Version and executable timestamp checks.
//!
//! Mirrors `verchk.cpp/.h` from WWLib.

#[cfg(target_os = "windows")]
use std::ffi::CString;
#[cfg(any(target_os = "windows", test))]
use std::io::{Read, Seek, SeekFrom};
#[cfg(target_os = "windows")]
use std::mem::size_of;
#[cfg(target_os = "windows")]
use std::os::windows::io::AsRawHandle;
#[cfg(target_os = "windows")]
use std::ptr;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{FILETIME, HANDLE, HINSTANCE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{
    GetFileTime, GetFileVersionInfoA, GetFileVersionInfoSizeA, VS_FIXEDFILEINFO, VerQueryValueA,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Diagnostics::Debug::IMAGE_FILE_HEADER;
#[cfg(target_os = "windows")]
use windows::Win32::System::SystemServices::IMAGE_DOS_HEADER;
#[cfg(target_os = "windows")]
use windows::core::PCSTR;

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
        // SAFETY: [Category 8 — FFI] queries version info size for a NUL-terminated path; out handle slot is valid.
        unsafe { GetFileVersionInfoSizeA(PCSTR(c_filename.as_ptr() as _), Some(&mut ver_handle)) };
    if ver_info_size == 0 {
        return false;
    }

    let mut buffer = vec![0u8; ver_info_size as usize];
    // SAFETY: [Category 8 — FFI] fills a local Vec sized exactly by the previous size query.
    let success = unsafe {
        GetFileVersionInfoA(
            PCSTR(c_filename.as_ptr() as _),
            Some(ver_handle),
            ver_info_size,
            buffer.as_mut_ptr() as _,
        )
    };
    if success.is_err() {
        return false;
    }

    let sub_block = CString::new("\\").unwrap();
    let mut data_ptr: *mut core::ffi::c_void = ptr::null_mut();
    let mut data_size: u32 = 0;
    // SAFETY: [Category 8 — FFI] queries the root sub-block of the buffer just filled; returned pointer/size are validated before use.
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

    // SAFETY: [Category 10 — OOB / Category 5 — invalid values] data_size was checked to equal size_of::<VS_FIXEDFILEINFO>(), so the read copies exactly one valid record.
    unsafe {
        *file_info = ptr::read(data_ptr as *const VS_FIXEDFILEINFO);
    }

    true
}

/// Retrieve the file's last-write time (despite the legacy function name).
#[cfg(target_os = "windows")]
pub fn get_file_creation_time(filename: &str, create_time: &mut FILETIME) -> bool {
    create_time.dwLowDateTime = 0;
    create_time.dwHighDateTime = 0;

    let file = match std::fs::File::open(filename) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let handle = HANDLE(file.as_raw_handle() as _);
    // SAFETY: [Category 8 — FFI] passes an open file handle; only the out timestamp slot is written.
    unsafe { GetFileTime(handle, None, None, Some(create_time)) }.is_ok()
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, PartialEq, Eq)]
struct PeFileHeader {
    machine: u16,
    number_of_sections: u16,
    time_date_stamp: u32,
    pointer_to_symbol_table: u32,
    number_of_symbols: u32,
    size_of_optional_header: u16,
    characteristics: u16,
}

// Read the COFF header at e_lfanew + 4, as the legacy implementation does.
#[cfg(any(target_os = "windows", test))]
fn read_pe_file_header(reader: &mut (impl Read + Seek)) -> Option<PeFileHeader> {
    let mut dos = [0u8; 64];
    reader.read_exact(&mut dos).ok()?;
    let offset = i32::from_le_bytes([dos[60], dos[61], dos[62], dos[63]]);
    let header_offset = u64::try_from(offset).ok()?.checked_add(4)?;
    reader.seek(SeekFrom::Start(header_offset)).ok()?;
    let mut header = [0u8; 20];
    reader.read_exact(&mut header).ok()?;
    Some(PeFileHeader {
        machine: u16::from_le_bytes([header[0], header[1]]),
        number_of_sections: u16::from_le_bytes([header[2], header[3]]),
        time_date_stamp: u32::from_le_bytes([header[4], header[5], header[6], header[7]]),
        pointer_to_symbol_table: u32::from_le_bytes([header[8], header[9], header[10], header[11]]),
        number_of_symbols: u32::from_le_bytes([header[12], header[13], header[14], header[15]]),
        size_of_optional_header: u16::from_le_bytes([header[16], header[17]]),
        characteristics: u16::from_le_bytes([header[18], header[19]]),
    })
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

    let header = match read_pe_file_header(&mut file) {
        Some(header) => header,
        None => return false,
    };
    file_header.Machine.0 = header.machine;
    file_header.NumberOfSections = header.number_of_sections;
    file_header.TimeDateStamp = header.time_date_stamp;
    file_header.PointerToSymbolTable = header.pointer_to_symbol_table;
    file_header.NumberOfSymbols = header.number_of_symbols;
    file_header.SizeOfOptionalHeader = header.size_of_optional_header;
    file_header.Characteristics.0 = header.characteristics;
    true
}

/// Read the image header from a loaded module.
#[cfg(target_os = "windows")]
pub fn get_image_file_header_from_instance(
    app_instance: HINSTANCE,
    file_header: &mut IMAGE_FILE_HEADER,
) -> bool {
    if app_instance.0.is_null() {
        return false;
    }

    // SAFETY: [Category 8 — FFI / Category 11 — provenance] HINSTANCE points at the process image's DOS header (Win32 contract); offsets follow the PE layout and are read unaligned as in verchk.cpp.
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
    let mut header1 = IMAGE_FILE_HEADER::default();
    let mut header2 = IMAGE_FILE_HEADER::default();
    let got_headers = get_image_file_header_from_instance(HINSTANCE(app_instance as *mut _), &mut header1)
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

#[cfg(test)]
mod tests {
    use super::{PeFileHeader, read_pe_file_header};
    use std::io::Cursor;

    fn pe_image() -> Vec<u8> {
        let mut image = vec![0u8; 128 + 4 + 20];
        image[..2].copy_from_slice(b"MZ");
        image[60..64].copy_from_slice(&128i32.to_le_bytes());
        image[128..132].copy_from_slice(b"PE\0\0");
        image[132..152].copy_from_slice(&[
            0x4c, 0x01, 0x03, 0x00, 0x78, 0x56, 0x34, 0xf2, 0xef, 0xcd, 0xab, 0x89,
            0x04, 0x03, 0x02, 0x01, 0xe0, 0x00, 0x02, 0x21,
        ]);
        image
    }

    #[test]
    fn decodes_pe_coff_header_in_little_endian() {
        assert_eq!(
            read_pe_file_header(&mut Cursor::new(pe_image())),
            Some(PeFileHeader {
                machine: 0x014c,
                number_of_sections: 3,
                time_date_stamp: 0xf2345678,
                pointer_to_symbol_table: 0x89abcdef,
                number_of_symbols: 0x01020304,
                size_of_optional_header: 0xe0,
                characteristics: 0x2102,
            })
        );
    }

    #[test]
    fn rejects_truncated_headers() {
        let image = pe_image();
        for length in 0..image.len() {
            assert_eq!(read_pe_file_header(&mut Cursor::new(&image[..length])), None);
        }
    }

    #[test]
    fn preserves_legacy_signature_and_offset_behavior() {
        let mut image = pe_image();
        image[0] = 0;
        image[128] = 0;
        assert!(read_pe_file_header(&mut Cursor::new(&image)).is_some());

        for offset in [0i32, 63] {
            image = pe_image();
            image[60..64].copy_from_slice(&offset.to_le_bytes());
            assert!(read_pe_file_header(&mut Cursor::new(&image)).is_some());
        }
        for offset in [-1i32, i32::MAX] {
            image = pe_image();
            image[60..64].copy_from_slice(&offset.to_le_bytes());
            assert_eq!(read_pe_file_header(&mut Cursor::new(&image)), None);
        }
    }
}
