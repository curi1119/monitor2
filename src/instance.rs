use std::{
    fs::{self, File, OpenOptions},
    os::windows::fs::OpenOptionsExt,
    path::Path,
};

// The open file denies sharing until process exit, including across login sessions.
// Keep the file itself: deleting it would race with another process acquiring it.
pub struct InstanceGuard {
    _file: File,
}
impl InstanceGuard {
    pub fn acquire() -> Result<Option<Self>, String> {
        let root = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATAがありません")?;
        Self::at(&std::path::PathBuf::from(root).join("monitor2/instance.lock"))
    }
    fn at(path: &Path) -> Result<Option<Self>, String> {
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(path)
        {
            Ok(file) => Ok(Some(Self { _file: file })),
            Err(error) if error.raw_os_error() == Some(32) => Ok(None), // ERROR_SHARING_VIOLATION
            Err(error) => Err(format!("二重起動の確認に失敗しました: {error}")),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excludes_second_instance_and_releases_on_drop() {
        let path = std::env::temp_dir().join(format!(
            "monitor2-instance-test-{}.lock",
            std::process::id()
        ));
        let first = InstanceGuard::at(&path).unwrap().unwrap();
        assert!(InstanceGuard::at(&path).unwrap().is_none());
        drop(first);
        let next = InstanceGuard::at(&path).unwrap().unwrap();
        drop(next);
        fs::remove_file(path).unwrap();
    }
}
