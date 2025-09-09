use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use dirs;

/// Удаляет UHPM и все его файлы, включая бинарник
pub fn self_remove() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path: PathBuf = env::current_exe()?; // путь к текущему бинарнику
    let home_dir = dirs::home_dir().ok_or("Не удалось получить HOME")?;
    let uhpm_dir = home_dir.join(".uhpm");

    // Создаём временный скрипт
    let tmp_script = home_dir.join("uhpm_uninstall.sh");
    let script_content = format!(
        r#"#!/bin/bash
# Ждём завершения процесса
sleep 1
rm -f "{}"
rm -rf "{}"
echo "UHPM удалён"
rm -- "$0"
"#,
        exe_path.to_string_lossy(),
        uhpm_dir.to_string_lossy()
    );

    fs::write(&tmp_script, script_content)?;
    fs::set_permissions(&tmp_script, fs::Permissions::from_mode(0o755))?;

    // Запускаем скрипт в фоне
    Command::new("bash")
        .arg(tmp_script)
        .spawn()?; // не ждём завершения текущего процесса

    Ok(())
}
