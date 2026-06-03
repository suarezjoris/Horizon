use std::process::Command;

pub async fn duckduckgo_search(query: &str) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let venv_python = home.join("Projects/Horizon/.venv/bin/python3");
    let script_path = home.join("Projects/Horizon/search_web.py");

    let output = Command::new(venv_python)
        .arg(script_path)
        .arg(query)
        .output()
        .map_err(|e| format!("Failed to run search script: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
