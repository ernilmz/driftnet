//! Narrow C ABI used by the signed Network Extension. No global policy state.
use drift_core::{Config, Decision, Flow};
use std::{ptr, slice, str};
const MAX_CONFIG: usize = 64 * 1024;
const MAX_IDENTITY: usize = 4096;
const MAX_HOST: usize = 1024;
pub struct Policy(Config);
unsafe fn text<'a>(data: *const u8, len: usize, limit: usize) -> Option<&'a str> {
    if data.is_null() || len > limit {
        return None;
    }
    // SAFETY: callers guarantee the readable byte range in the exported contract.
    str::from_utf8(unsafe { slice::from_raw_parts(data, len) }).ok()
}
/// Create a validated immutable policy. Returns null on invalid input.
/// # Safety
/// `data` must point to `len` readable bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn drift_policy_create(data: *const u8, len: usize) -> *mut Policy {
    // SAFETY: guaranteed by the caller; limits/UTF-8 are checked before parsing.
    let Some(json) = (unsafe { text(data, len, MAX_CONFIG) }) else {
        return ptr::null_mut();
    };
    let Ok(config) = serde_json::from_str::<Config>(json) else {
        return ptr::null_mut();
    };
    if config.rules.len() > 512 || config.validate().is_err() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(Policy(config)))
}
/// Release exactly one policy created by drift_policy_create.
/// # Safety
/// `policy` must be null or a live allocation returned by drift_policy_create,
/// with no concurrent readers and no prior release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn drift_policy_destroy(policy: *mut Policy) {
    if !policy.is_null() {
        // SAFETY: ownership is transferred back by the caller exactly once.
        drop(unsafe { Box::from_raw(policy) });
    }
}
/// Return 1 only for an explicitly selected flow. Invalid/unknown metadata is direct.
/// # Safety
/// `policy` must remain live for the duration of the call. Every non-null byte
/// pointer must refer to its declared readable length. Concurrent reads are safe.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn drift_policy_selected(
    policy: *const Policy,
    identity: *const u8,
    identity_len: usize,
    hostname: *const u8,
    hostname_len: usize,
) -> u8 {
    if policy.is_null() {
        return 0;
    }
    // SAFETY: pointers are borrowed only within this call, as contracted above.
    let policy = unsafe { &*policy };
    let app_identity = unsafe { text(identity, identity_len, MAX_IDENTITY) };
    let hostname = unsafe { text(hostname, hostname_len, MAX_HOST) };
    matches!(
        policy.0.decide(&Flow {
            app_identity,
            hostname,
            verified_discord_installation: false
        }),
        Decision::Selected { .. }
    ) as u8
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_abi_validates_and_preserves_selection_boundaries() {
        let mut c = Config::default();
        c.add_site("example.com").unwrap();
        let bytes = serde_json::to_vec(&c).unwrap();
        // SAFETY: these buffers and the policy remain live until explicitly freed.
        unsafe {
            let p = drift_policy_create(bytes.as_ptr(), bytes.len());
            assert!(!p.is_null());
            let app = b"macos:com.hnc.Discord";
            assert_eq!(
                drift_policy_selected(p, app.as_ptr(), app.len(), ptr::null(), 0),
                1
            );
            for (host, expected) in [
                ("example.com", 1),
                ("media.example.com", 1),
                ("notexample.com", 0),
                ("example.com.attacker.net", 0),
            ] {
                assert_eq!(
                    drift_policy_selected(p, ptr::null(), 0, host.as_ptr(), host.len()),
                    expected
                );
            }
            assert_eq!(drift_policy_selected(p, ptr::null(), 0, ptr::null(), 0), 0);
            drift_policy_destroy(p);
        }
    }
    #[test]
    fn c_abi_rejects_malformed_and_oversized_configuration() {
        // SAFETY: null is explicitly supported, and the other inputs are live slices.
        unsafe {
            assert!(drift_policy_create(ptr::null(), 0).is_null());
            assert!(drift_policy_create(b"{}".as_ptr(), 2).is_null());
            let bad = [255u8];
            assert!(drift_policy_create(bad.as_ptr(), 1).is_null());
            // Oversized lengths are rejected before dereferencing even one byte.
            assert!(drift_policy_create(b"x".as_ptr(), MAX_CONFIG + 1).is_null());
            drift_policy_destroy(ptr::null_mut());
        }
    }
}
