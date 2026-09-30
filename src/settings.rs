use crate::hardware::wide;
use std::{
    fs,
    path::PathBuf,
    sync::{Condvar, Mutex},
    time::Duration,
};
use windows_sys::{Win32::System::Registry::*, core::w};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub topmost: bool,
    pub autostart: bool,
    pub interval_ms: u32,
    pub physical_cores: bool,
    pub show_core_percent: bool,
    pub show_core_numbers: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            topmost: true,
            autostart: false,
            interval_ms: 1000,
            physical_cores: false,
            show_core_percent: false,
            show_core_numbers: true,
        }
    }
}
impl Settings {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut result = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err("設定ファイルの形式が不正です".into());
            };
            let value = value.trim();
            let boolean = || {
                value
                    .parse::<bool>()
                    .map_err(|_| format!("{key}: true/falseを指定してください"))
            };
            match key.trim() {
                "topmost" => result.topmost = boolean()?,
                "autostart" => result.autostart = boolean()?,
                "interval_ms" => {
                    result.interval_ms = value.parse().map_err(|_| "更新間隔が不正です")?
                }
                "physical_cores" => result.physical_cores = boolean()?,
                "show_core_percent" => result.show_core_percent = boolean()?,
                "show_core_numbers" => result.show_core_numbers = boolean()?,
                _ => {} // Preserve forward compatibility with newer configuration keys.
            }
        }
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(250..=60_000).contains(&self.interval_ms) {
            return Err("監視インターバルは0.25〜60秒で指定してください".into());
        }
        Ok(())
    }
    fn encode(&self) -> String {
        format!(
            "topmost={}\nautostart={}\ninterval_ms={}\nphysical_cores={}\nshow_core_percent={}\nshow_core_numbers={}\n",
            self.topmost,
            self.autostart,
            self.interval_ms,
            self.physical_cores,
            self.show_core_percent,
            self.show_core_numbers
        )
    }
    pub fn load() -> Result<Self, String> {
        let path = config_path()?;
        let mut settings = match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(format!("設定を読み込めません: {error}")),
        };
        settings.autostart = autostart_enabled();
        Ok(settings)
    }
    pub fn save(&self) -> Result<(), String> {
        self.validate()?;
        let path = config_path()?;
        let previous = fs::read(&path).ok();
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        atomic_save(&path, self.encode().as_bytes())?;
        if let Err(error) = set_autostart(self.autostart) {
            if let Some(bytes) = previous {
                let _ = atomic_save(&path, &bytes);
            } else {
                let _ = fs::remove_file(&path);
            }
            return Err(error);
        }
        Ok(())
    }
}
fn atomic_save(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    fs::rename(&temporary, path).map_err(|e| e.to_string())
}
fn config_path() -> Result<PathBuf, String> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATAがありません")?)
            .join("monitor2/settings.ini"),
    )
}
const RUN_KEY: windows_sys::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
fn autostart_enabled() -> bool {
    let mut buffer = [0u16; 32768];
    let mut bytes = std::mem::size_of_val(&buffer) as u32;
    // SAFETY: current-user registry read into a bounded UTF-16 buffer.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            w!("monitor2"),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if result != 0 {
        return false;
    }
    let expected = std::env::current_exe()
        .ok()
        .map(|p| format!("\"{}\"", p.display()));
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    expected.is_some_and(|e| String::from_utf16_lossy(&buffer[..end]) == e)
}
fn set_autostart(enabled: bool) -> Result<(), String> {
    let mut key = std::ptr::null_mut();
    // SAFETY: open only this user's Run key; owned handle is closed on every path.
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(format!("自動起動設定を開けません: {status}"));
    }
    let status = if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string());
        match exe {
            Ok(exe) => {
                let value = wide(&format!("\"{}\"", exe.display()));
                unsafe {
                    RegSetValueExW(
                        key,
                        w!("monitor2"),
                        0,
                        REG_SZ,
                        value.as_ptr().cast(),
                        (value.len() * 2) as u32,
                    )
                }
            }
            Err(error) => {
                unsafe {
                    RegCloseKey(key);
                }
                return Err(error);
            }
        }
    } else {
        unsafe { RegDeleteValueW(key, w!("monitor2")) }
    };
    unsafe {
        RegCloseKey(key);
    }
    if status != 0 && !(status == 2 && !enabled) {
        return Err(format!("自動起動設定を保存できません: {status}"));
    }
    Ok(())
}
pub struct SharedSettings {
    pub value: Mutex<Settings>,
    pub signal: Mutex<(bool, u64)>,
    pub wake: Condvar,
}
impl SharedSettings {
    pub fn new(settings: Settings) -> Self {
        Self {
            value: Mutex::new(settings),
            signal: Mutex::new((false, 0)),
            wake: Condvar::new(),
        }
    }
    pub fn get(&self) -> Settings {
        self.value.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn apply(&self, settings: Settings) -> Result<(), String> {
        settings.save()?;
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = settings;
        self.signal.lock().unwrap_or_else(|e| e.into_inner()).1 += 1;
        self.wake.notify_one();
        Ok(())
    }
    pub fn wait(&self) -> bool {
        let signal = self.signal.lock().unwrap_or_else(|e| e.into_inner());
        let revision = signal.1;
        let duration = Duration::from_millis(self.get().interval_ms as u64);
        let (signal, _) = self
            .wake
            .wait_timeout_while(signal, duration, |s| !s.0 && s.1 == revision)
            .unwrap_or_else(|e| e.into_inner());
        signal.0
    }
    pub fn stop(&self) {
        self.signal.lock().unwrap_or_else(|e| e.into_inner()).0 = true;
        self.wake.notify_one();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_round_trip_and_unknown_keys() {
        let s = Settings {
            topmost: false,
            autostart: true,
            interval_ms: 250,
            physical_cores: true,
            show_core_percent: true,
            show_core_numbers: false,
        };
        assert_eq!(
            Settings::parse(&(s.encode() + "future_key=value\n")).unwrap(),
            s
        );
    }
    #[test]
    fn invalid_intervals_and_booleans_are_rejected() {
        assert!(Settings::parse("interval_ms=0\n").is_err());
        assert!(Settings::parse("interval_ms=60001\n").is_err());
        assert!(Settings::parse("topmost=maybe\n").is_err());
    }
}
