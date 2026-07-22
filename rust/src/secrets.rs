#![allow(unsafe_code)]
// Windows-only: DPAPI LocalMachine so blobs decrypt under SYSTEM.
#[cfg(windows)]
pub use imp::*;

// ponytail: only referenced from notify.rs's #[cfg(windows)] paths; cfg-gating
// here (rather than a blanket allow(dead_code)) keeps Linux builds warning-free.
#[cfg(windows)]
pub const SMTP_ENTROPY: &[u8] = b"ProcDumpMonitor-SMTP-v1";
#[cfg(windows)]
pub const WEBHOOK_ENTROPY: &[u8] = b"ProcDumpMonitor-Webhook-v1";

#[cfg(windows)]
mod imp {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
        CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN,
    };

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    /// Encrypt with DPAPI LocalMachine; returns base64. Empty input -> "".
    // ponytail: called when the GUI wizard saves a SMTP password / webhook
    // URL (Task 9); the monitor only ever decrypts (unprotect), so a release
    // build has no caller yet.
    #[allow(dead_code)]
    pub fn protect(plain: &str, entropy: &[u8]) -> String {
        if plain.is_empty() { return String::new(); }
        let input = blob(plain.as_bytes());
        let ent = blob(entropy);
        let mut out = CRYPT_INTEGER_BLOB::default();
        let ok = unsafe {
            CryptProtectData(&input, None, Some(&ent), None, None,
                CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        if ok.is_err() { return String::new(); }
        let bytes = unsafe {
            std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec()
        };
        unsafe { let _ = LocalFree(HLOCAL(out.pbData as *mut core::ffi::c_void)); }
        B64.encode(bytes)
    }

    /// Decrypt a base64 DPAPI blob; "" on any failure (matches C#).
    pub fn unprotect(b64: &str, entropy: &[u8]) -> String {
        if b64.is_empty() { return String::new(); }
        let Ok(encrypted) = B64.decode(b64) else { return String::new(); };
        let input = blob(&encrypted);
        let ent = blob(entropy);
        let mut out = CRYPT_INTEGER_BLOB::default();
        let ok = unsafe {
            CryptUnprotectData(&input, None, Some(&ent), None, None,
                CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        if ok.is_err() { return String::new(); }
        let bytes = unsafe {
            std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec()
        };
        unsafe { let _ = LocalFree(HLOCAL(out.pbData as *mut core::ffi::c_void)); }
        String::from_utf8(bytes).unwrap_or_default()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn roundtrip_and_wrong_entropy_fails() {
            let blob = protect("hunter2", super::super::SMTP_ENTROPY);
            assert!(!blob.is_empty());
            assert_eq!(unprotect(&blob, super::super::SMTP_ENTROPY), "hunter2");
            assert_eq!(unprotect(&blob, b"wrong-entropy"), "");
            assert_eq!(unprotect("not-base64!!!", super::super::SMTP_ENTROPY), "");
        }
    }
}
