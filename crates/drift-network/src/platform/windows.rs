use serde::{Deserialize, Serialize};
use std::{ffi::c_void, io, path::PathBuf, ptr};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
    Networking::WinInet::{
        INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED, InternetSetOptionW,
    },
    System::Registry::*,
};
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
struct Key(HKEY);
impl Key {
    fn open() -> io::Result<Self> {
        let mut key = ptr::null_mut();
        let name = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        // SAFETY: name is NUL terminated and key is a valid output pointer.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                name.as_ptr(),
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut key,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        Ok(Self(key))
    }
    fn read(&self, name: &str) -> io::Result<Option<Value>> {
        let name = wide(name);
        let mut kind = 0;
        let mut length = 0;
        // SAFETY: live key, terminated name, valid scalar output pointers.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                name.as_ptr(),
                ptr::null(),
                &mut kind,
                ptr::null_mut(),
                &mut length,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS || length > 65536 {
            return Err(io::Error::other(
                "Could not read the existing Windows proxy settings",
            ));
        }
        let mut bytes = vec![0; length as usize];
        // SAFETY: buffer length equals the queried capacity, outputs are valid.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                name.as_ptr(),
                ptr::null(),
                &mut kind,
                bytes.as_mut_ptr(),
                &mut length,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        bytes.truncate(length as usize);
        Ok(Some(Value { kind, bytes }))
    }
    fn write(&self, name: &str, value: Option<&Value>) -> io::Result<()> {
        let name = wide(name);
        // SAFETY: key/name/buffer are live and declared lengths match allocation.
        let status = unsafe {
            match value {
                Some(v) => RegSetValueExW(
                    self.0,
                    name.as_ptr(),
                    0,
                    v.kind,
                    v.bytes.as_ptr(),
                    v.bytes.len() as u32,
                ),
                None => RegDeleteValueW(self.0, name.as_ptr()),
            }
        };
        if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        Ok(())
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
#[derive(Serialize, Deserialize, PartialEq)]
struct Value {
    kind: u32,
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    ours: Value,
    previous: Option<Value>,
}
fn journal_path() -> io::Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| io::Error::other("No local app data folder"))?,
    )
    .join("Drift/proxy-journal.json"))
}
fn refresh() {
    unsafe {
        InternetSetOptionW(
            ptr::null::<c_void>(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            ptr::null(),
            0,
        );
        InternetSetOptionW(
            ptr::null::<c_void>(),
            INTERNET_OPTION_REFRESH,
            ptr::null(),
            0,
        );
    }
}
pub struct ProxyLease {
    key: Key,
    journal: Journal,
    path: PathBuf,
}
impl ProxyLease {
    pub fn recover() -> io::Result<()> {
        let path = journal_path()?;
        if !path.exists() {
            return Ok(());
        }
        let journal: Journal = serde_json::from_slice(&std::fs::read(&path)?)?;
        let key = Key::open()?;
        if key.read("AutoConfigURL")?.as_ref() == Some(&journal.ours) {
            key.write("AutoConfigURL", journal.previous.as_ref())?;
            refresh();
        }
        std::fs::remove_file(path)?;
        Ok(())
    }
    pub fn enable(url: &str) -> io::Result<Self> {
        Self::recover()?;
        let parsed = url::Url::parse(url).map_err(io::Error::other)?;
        if parsed.scheme() != "http"
            || parsed.host_str() != Some("127.0.0.1")
            || parsed.port().is_none_or(|p| p < 1024)
            || parsed.path() != "/proxy.pac"
        {
            return Err(io::Error::other("Invalid local proxy endpoint"));
        }
        let key = Key::open()?;
        let previous = key.read("AutoConfigURL")?;
        let auto_active = previous
            .as_ref()
            .is_some_and(|v| v.bytes.iter().any(|b| *b != 0));
        let manual = key
            .read("ProxyEnable")?
            .is_some_and(|v| v.bytes.first().is_some_and(|b| *b != 0));
        if auto_active || manual {
            return Err(io::Error::other(
                "Another system proxy is active. Drift kept it unchanged.",
            ));
        }
        let ours = Value {
            kind: REG_SZ,
            bytes: wide(url).iter().flat_map(|v| v.to_le_bytes()).collect(),
        };
        let journal = Journal { ours, previous };
        let path = journal_path()?;
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        use std::io::Write;
        temp.write_all(&serde_json::to_vec(&journal)?)?;
        temp.as_file().sync_all()?;
        temp.persist(&path).map_err(|e| e.error)?;
        key.write("AutoConfigURL", Some(&journal.ours))?;
        refresh();
        Ok(Self { key, journal, path })
    }
}
impl Drop for ProxyLease {
    fn drop(&mut self) {
        if self.key.read("AutoConfigURL").ok().flatten().as_ref() == Some(&self.journal.ours) {
            if self
                .key
                .write("AutoConfigURL", self.journal.previous.as_ref())
                .is_err()
            {
                return;
            }
            refresh();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}
