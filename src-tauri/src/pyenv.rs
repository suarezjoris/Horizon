use std::path::{Path, PathBuf};

pub fn venv_python(venv_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python3")
    }
}

pub fn system_python() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}
