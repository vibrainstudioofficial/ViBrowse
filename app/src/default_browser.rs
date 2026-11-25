use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(target_os = "windows")]
fn run_reg(args: &[&str]) -> bool {
    let mut cmd = Command::new("reg.exe");
    cmd.args(args).creation_flags(CREATE_NO_WINDOW);
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Registers ViBrowse in HKCU so it appears in Windows Settings -> Default Apps (no Admin/UAC needed).
#[cfg(target_os = "windows")]
pub fn register_browser() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let exe_str = exe.to_string_lossy().to_string();
    let open_cmd = format!("\"{}\" \"%1\"", exe_str);
    let icon_val = format!("{},0", exe_str);

    let entries: &[(&str, Option<&str>, &str)] = &[
        (
            r"HKCU\Software\Classes\ViBrowseHTML",
            None,
            "ViBrowse HTML Document",
        ),
        (
            r"HKCU\Software\Classes\ViBrowseHTML\DefaultIcon",
            None,
            &icon_val,
        ),
        (
            r"HKCU\Software\Classes\ViBrowseHTML\shell\open\command",
            None,
            &open_cmd,
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse",
            None,
            "ViBrowse",
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\DefaultIcon",
            None,
            &icon_val,
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\shell\open\command",
            None,
            &open_cmd,
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\Capabilities",
            Some("ApplicationName"),
            "ViBrowse",
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\Capabilities",
            Some("ApplicationDescription"),
            "ViBrowse - Lightweight Rust Desktop Browser",
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\Capabilities\URLAssociations",
            Some("http"),
            "ViBrowseHTML",
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\Capabilities\URLAssociations",
            Some("https"),
            "ViBrowseHTML",
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\Capabilities\FileAssociations",
            Some(".htm"),
            "ViBrowseHTML",
        ),
        (
            r"HKCU\Software\Clients\StartMenuInternet\ViBrowse\Capabilities\FileAssociations",
            Some(".html"),
            "ViBrowseHTML",
        ),
        (
            r"HKCU\Software\RegisteredApplications",
            Some("ViBrowse"),
            r"Software\Clients\StartMenuInternet\ViBrowse\Capabilities",
        ),
    ];

    for (key, val_name, data) in entries {
        match val_name {
            Some(name) => {
                run_reg(&["add", key, "/v", name, "/t", "REG_SZ", "/d", data, "/f"]);
            }
            None => {
                run_reg(&["add", key, "/ve", "/t", "REG_SZ", "/d", data, "/f"]);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn register_browser() {}

/// Checks whether ViBrowse is currently the OS default browser for HTTPS links.
pub fn is_default_browser() -> bool {
    #[cfg(target_os = "windows")]
    {
        // Windows 11 24H2+ stores the active ProgId under `UserChoiceLatest\ProgId`
        // while keeping legacy `UserChoice` at the previous browser. Check Latest first!
        let keys = [
            r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoiceLatest\ProgId",
            r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
        ];
        for key in keys {
            let mut cmd = Command::new("reg.exe");
            cmd.args(["query", key, "/v", "ProgId"])
                .creation_flags(CREATE_NO_WINDOW);
            if let Ok(out) = cmd.output() {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    if stdout.contains("progid") {
                        return stdout.contains("vibrowse");
                    }
                }
            }
        }
        false
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-settings")
            .args(["get", "default-web-browser"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .to_lowercase()
                    .contains("vibrowse")
            })
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

/// Opens OS Default Apps settings for ViBrowse on Windows, or sets default directly on Linux.
pub fn set_or_open_default_browser() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        register_browser();
        let mut cmd = Command::new("cmd.exe");
        cmd.args([
            "/C",
            "start",
            "",
            "ms-settings:defaultapps?registeredAppUser=ViBrowse",
        ])
        .creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("Gagal membuka Windows Default Apps: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let status = Command::new("xdg-settings")
            .args(["set", "default-web-browser", "vibrowse.desktop"])
            .status()
            .map_err(|e| format!("Gagal menjalankan xdg-settings: {}", e))?;
        if status.success() {
            Ok(())
        } else {
            Err("xdg-settings gagal mengubah default browser".to_string())
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err("Pengaturan default browser belum didukung di OS ini".to_string())
    }
}
