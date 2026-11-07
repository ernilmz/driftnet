//! Minisign verification shared by the updater and its offline regression tests.
use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};
use std::{fs::File, io::Read, path::Path};

const PUBLIC_KEY: &str = include_str!("../resources/updater-public-key.txt");

fn decode_envelope(value: &str) -> Result<String, String> {
    if value.trim_start().starts_with("untrusted comment:") {
        return Ok(value.trim().to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|_| "Invalid update signature encoding")?;
    String::from_utf8(bytes).map_err(|_| "Invalid update signature text".into())
}

pub fn verify_file(path: &Path, encoded_signature: &str, version: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    verify_reader(&mut file, encoded_signature, version)
}

pub fn verify_bytes(bytes: &[u8], encoded_signature: &str, version: &str) -> Result<(), String> {
    verify_reader(&mut std::io::Cursor::new(bytes), encoded_signature, version)
}

fn verify_reader(
    reader: &mut impl Read,
    encoded_signature: &str,
    version: &str,
) -> Result<(), String> {
    let public = decode_envelope(PUBLIC_KEY)?;
    let public_line = public.lines().nth(1).ok_or("Missing update public key")?;
    let key = PublicKey::from_base64(public_line).map_err(|_| "Invalid update public key")?;
    let signature = Signature::decode(&decode_envelope(encoded_signature)?)
        .map_err(|_| "Invalid update signature")?;
    let mut verifier = key
        .verify_stream(&signature)
        .map_err(|_| "Update signature does not support secure streaming verification")?;
    let mut bytes = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut bytes).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        verifier.update(&bytes[..read]);
    }
    verifier
        .finalize()
        .map_err(|_| "The update signature is invalid. The installer was not opened.")?;
    // Tauri's --app-version creates this authenticated trusted-comment field.
    // Requiring it prevents pairing an old signed installer with a new manifest.
    let versions: Vec<_> = signature
        .trusted_comment()
        .split('\t')
        .filter_map(|part| part.strip_prefix("version:"))
        .collect();
    if versions != [version] {
        return Err("The signed installer version does not match the update manifest.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const BYTES: &[u8] = include_bytes!("../../../tests/updater/fixture.pkg");
    const SIGNATURE: &str = include_str!("../../../tests/updater/fixture.pkg.sig");
    const MANIFEST: &[u8] = include_bytes!("../../../tests/updater/manifest.json");
    const MANIFEST_SIGNATURE: &str = include_str!("../../../tests/updater/manifest.json.sig");
    fn fixture(bytes: &[u8]) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        file
    }
    #[test]
    fn accepts_the_complete_signed_installer_for_its_signed_version() {
        let file = fixture(BYTES);
        verify_file(file.path(), SIGNATURE, "0.0.0-fixture").unwrap();
        verify_bytes(BYTES, SIGNATURE, "0.0.0-fixture").unwrap();
    }
    #[test]
    fn rejects_modified_installer_bytes() {
        let mut bytes = BYTES.to_vec();
        bytes[0] ^= 1;
        let file = fixture(&bytes);
        assert!(verify_file(file.path(), SIGNATURE, "0.0.0-fixture").is_err());
    }
    #[test]
    fn rejects_an_old_installer_relabelled_as_a_new_version() {
        let file = fixture(BYTES);
        assert!(verify_file(file.path(), SIGNATURE, "10.0.0").is_err());
    }
    #[test]
    fn rejects_a_forged_trusted_version_comment() {
        let file = fixture(BYTES);
        let forged = decode_envelope(SIGNATURE)
            .unwrap()
            .replace("version:0.0.0-fixture", "version:10.0.0");
        assert!(verify_file(file.path(), &forged, "10.0.0").is_err());
    }
    #[test]
    fn accepts_authenticated_platform_metadata() {
        verify_bytes(MANIFEST, MANIFEST_SIGNATURE, "0.0.0-fixture").unwrap();
    }
    #[test]
    fn rejects_platform_or_url_tampering_in_metadata() {
        let forged = String::from_utf8(MANIFEST.to_vec())
            .unwrap()
            .replace("darwin-aarch64", "darwin-x86_64");
        assert!(verify_bytes(forged.as_bytes(), MANIFEST_SIGNATURE, "0.0.0-fixture").is_err());
        let forged = String::from_utf8(MANIFEST.to_vec())
            .unwrap()
            .replace("fixture.pkg", "other-architecture.pkg");
        assert!(verify_bytes(forged.as_bytes(), MANIFEST_SIGNATURE, "0.0.0-fixture").is_err());
    }
}
