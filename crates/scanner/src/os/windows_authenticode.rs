//! Authenticode signer lookup (Windows only). Contains `unsafe` FFI.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Security::Cryptography::{
    CERT_CONTEXT, CERT_FIND_SUBJECT_CERT, CERT_INFO, CERT_NAME_SIMPLE_DISPLAY_TYPE,
    CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED, CERT_QUERY_ENCODING_TYPE,
    CERT_QUERY_FORMAT_FLAG_BINARY, CERT_QUERY_OBJECT_FILE, CMSG_SIGNER_INFO,
    CMSG_SIGNER_INFO_PARAM, CertCloseStore, CertFindCertificateInStore, CertFreeCertificateContext,
    CertGetNameStringW, CryptMsgClose, CryptMsgGetParam, CryptQueryObject, HCERTSTORE,
    PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
};
use windows_sys::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
    WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE, WinVerifyTrust,
};

/// Returns the signer display name of `path` only when its Authenticode signature is valid.
pub(super) fn authenticode_publisher(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    if !signature_is_valid(&wide) {
        return None;
    }
    signer_name(&wide)
}

/// Runs `WinVerifyTrust` (verify, then always close). `wide` is NUL-terminated.
fn signature_is_valid(wide: &[u16]) -> bool {
    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide.as_ptr(),
        hFile: std::ptr::null_mut(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..WINTRUST_DATA::default()
    };
    data.Anonymous.pFile = &mut file_info;
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // INVALID_HANDLE_VALUE: no parent window, no UI.
    let hwnd = -1isize as *mut c_void;
    // SAFETY: `data` is a fully initialised WINTRUST_DATA whose `pFile` points to
    // `file_info`, which points to the NUL-terminated `wide`; all three outlive the call.
    let status =
        unsafe { WinVerifyTrust(hwnd, &mut action, (&mut data as *mut WINTRUST_DATA).cast()) };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // SAFETY: same `data` (now carrying the state handle returned by the verify call);
    // the CLOSE action releases that state.
    unsafe { WinVerifyTrust(hwnd, &mut action, (&mut data as *mut WINTRUST_DATA).cast()) };
    status == 0
}

/// Owns a certificate store handle.
struct StoreGuard(HCERTSTORE);
impl Drop for StoreGuard {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful CryptQueryObject and is closed once.
        unsafe { CertCloseStore(self.0, 0) };
    }
}

/// Owns a crypto message handle.
struct MsgGuard(*mut c_void);
impl Drop for MsgGuard {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful CryptQueryObject and is closed once.
        unsafe { CryptMsgClose(self.0) };
    }
}

/// Owns a certificate context.
struct CertGuard(*mut CERT_CONTEXT);
impl Drop for CertGuard {
    fn drop(&mut self) {
        // SAFETY: the context came from CertFindCertificateInStore and is freed once.
        unsafe { CertFreeCertificateContext(self.0) };
    }
}

/// Extracts the display name of the signer certificate embedded in the file.
fn signer_name(wide: &[u16]) -> Option<String> {
    let mut encoding: CERT_QUERY_ENCODING_TYPE = 0;
    let mut store: HCERTSTORE = std::ptr::null_mut();
    let mut msg: *mut c_void = std::ptr::null_mut();
    // SAFETY: `wide` is NUL-terminated and outlives the call; every out pointer is a valid
    // writable local or null where the API allows it.
    let ok = unsafe {
        CryptQueryObject(
            CERT_QUERY_OBJECT_FILE,
            wide.as_ptr().cast(),
            CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
            CERT_QUERY_FORMAT_FLAG_BINARY,
            0,
            &mut encoding,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut store,
            &mut msg,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return None;
    }
    let store = StoreGuard(store);
    let msg = MsgGuard(msg);

    let mut size: u32 = 0;
    // SAFETY: `msg.0` is a valid message handle; a null buffer asks for the required size.
    let ok = unsafe {
        CryptMsgGetParam(
            msg.0,
            CMSG_SIGNER_INFO_PARAM,
            0,
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if ok == 0 || (size as usize) < std::mem::size_of::<CMSG_SIGNER_INFO>() {
        return None;
    }
    // u64 elements guarantee 8-byte alignment for CMSG_SIGNER_INFO.
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: `buf` is writable for at least `size` bytes.
    let ok = unsafe {
        CryptMsgGetParam(
            msg.0,
            CMSG_SIGNER_INFO_PARAM,
            0,
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if ok == 0 {
        return None;
    }
    // SAFETY: the buffer holds a CMSG_SIGNER_INFO written by the API and is suitably
    // aligned; its inner pointers reference data inside `buf`, which outlives their use.
    let signer = unsafe { &*(buf.as_ptr().cast::<CMSG_SIGNER_INFO>()) };

    // Only Issuer and SerialNumber are read by CERT_FIND_SUBJECT_CERT.
    let info = CERT_INFO {
        Issuer: signer.Issuer,
        SerialNumber: signer.SerialNumber,
        ..CERT_INFO::default()
    };
    // SAFETY: `store.0` is a valid store; `info` is a valid CERT_INFO whose blobs point
    // into `buf`, alive for the call.
    let cert = unsafe {
        CertFindCertificateInStore(
            store.0,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            0,
            CERT_FIND_SUBJECT_CERT,
            (&info as *const CERT_INFO).cast(),
            std::ptr::null(),
        )
    };
    if cert.is_null() {
        return None;
    }
    let cert = CertGuard(cert);

    // SAFETY: `cert.0` is a valid certificate context; a null buffer asks for the length
    // (in characters, including the terminating NUL).
    let len = unsafe {
        CertGetNameStringW(
            cert.0,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
        )
    };
    if len <= 1 {
        return None;
    }
    let mut name = vec![0u16; len as usize];
    // SAFETY: `name` is writable for exactly `len` UTF-16 units.
    let written = unsafe {
        CertGetNameStringW(
            cert.0,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            std::ptr::null(),
            name.as_mut_ptr(),
            len,
        )
    };
    if written <= 1 {
        return None;
    }
    name.truncate(written as usize - 1);
    let text = String::from_utf16_lossy(&name);
    // `store` and `msg` drop after `cert`, in reverse declaration order.
    drop(store);
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ignored by default: many System32 binaries are *catalog*-signed (no embedded
    // signature), so they legitimately return `None` here. Run manually during M7 with
    // `cargo test -p gmm-scanner -- --ignored` and, if it fails, point it at a binary with an
    // embedded signature (e.g. a signed game DLL). See docs/windows-validation.md (A-06).
    #[test]
    #[ignore = "M7 manual check: depends on the binary having an embedded signature"]
    fn microsoft_binary_is_signed_by_microsoft() {
        let name = authenticode_publisher(Path::new(r"C:\Windows\System32\kernel32.dll"));
        assert!(name.is_some_and(|n| n.contains("Microsoft")));
    }

    #[test]
    fn random_bytes_have_no_publisher() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let file = dir.path().join("random.dll");
        let bytes: Vec<u8> = (0..4096u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        std::fs::write(&file, bytes).expect("write");
        assert_eq!(authenticode_publisher(&file), None);
    }

    #[test]
    fn missing_file_has_no_publisher() {
        assert_eq!(
            authenticode_publisher(Path::new(r"C:\definitely\missing.dll")),
            None
        );
    }
}
