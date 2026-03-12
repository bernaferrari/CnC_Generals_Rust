//! Constants mirroring WWDownload `ftpdefs.h`.

const SEVERITY_ERROR: u32 = 1;
const FACILITY_ITF: u32 = 4;

const fn make_hresult(severity: u32, facility: u32, code: u32) -> u32 {
    (severity << 31) | (facility << 16) | code
}

pub const FTP_SUCCEEDED: u32 = 0;
pub const FTP_FAILED: u32 = make_hresult(SEVERITY_ERROR, FACILITY_ITF, 1);
pub const FTP_TRYING: u32 = make_hresult(SEVERITY_ERROR, FACILITY_ITF, 2);
