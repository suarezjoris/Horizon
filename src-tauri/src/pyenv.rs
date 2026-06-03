use std::path::{Path, PathBuf};

/// Resolve the Python interpreter inside a virtualenv, cross-platform.
/// Windows venvs put the interpreter in `Scripts\python.exe`; Unix in `bin/python3`.
pub fn venv_python(venv_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python3")
    }
}

/// Name of the system Python interpreter on PATH, cross-platform.
pub fn system_python() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}
