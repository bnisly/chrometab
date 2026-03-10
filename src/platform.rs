// Platform-specific browser window management

use anyhow::Result;
use crate::chrome::VersionInfo;

/// Browser variant for window activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserKind {
    Chrome,
    Brave,
}

impl BrowserKind {
    #[cfg(target_os = "windows")]
    pub fn window_title_substring(self) -> &'static str {
        match self {
            BrowserKind::Chrome => "Chrome",
            BrowserKind::Brave => "Brave",
        }
    }

    #[cfg(target_os = "macos")]
    pub fn app_name(self) -> &'static str {
        match self {
            BrowserKind::Chrome => "Google Chrome",
            BrowserKind::Brave => "Brave Browser",
        }
    }

    #[cfg(target_os = "linux")]
    pub fn window_name(self) -> &'static str {
        match self {
            BrowserKind::Chrome => "Google Chrome",
            BrowserKind::Brave => "Brave",
        }
    }
}

#[cfg(target_os = "windows")]
pub fn bring_browser_to_front(browser: BrowserKind) -> Result<()> {
    use std::ptr::null_mut;
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::*;

    let substring = browser.window_title_substring();

    unsafe {
        struct State {
            hwnd: HWND,
            substring: &'static str,
        }
        let mut state = State { hwnd: null_mut(), substring };

        unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: isize) -> i32 {
            let state = &mut *(lparam as *mut State);
            let mut text: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, text.as_mut_ptr(), 512);
            if len > 0 {
                let title = String::from_utf16_lossy(&text[..len as usize]);
                if title.contains(state.substring) && IsWindowVisible(hwnd) != 0 {
                    state.hwnd = hwnd;
                    return 0;
                }
            }
            1
        }

        EnumWindows(Some(enum_cb), &mut state as *mut State as isize);

        if !state.hwnd.is_null() {
            ShowWindow(state.hwnd, SW_RESTORE);
            ShowWindow(state.hwnd, SW_SHOW);
            SetForegroundWindow(state.hwnd);
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn bring_browser_to_front(browser: BrowserKind) -> Result<()> {
    let app = browser.app_name();
    std::process::Command::new("osascript")
        .args(["-e", &format!("tell application \"{}\" to activate", app)])
        .output()?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn bring_browser_to_front(browser: BrowserKind) -> Result<()> {
    let name = browser.window_name();
    std::process::Command::new("xdotool")
        .args(["search", "--name", name, "windowactivate"])
        .output()?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn bring_browser_to_front(_browser: BrowserKind) -> Result<()> {
    Ok(())
}

/// Resolve BrowserKind from CLI --browser value and optional CDP version info.
pub fn resolve_browser(cli_browser: &str, version_info: Option<&VersionInfo>) -> BrowserKind {
    match cli_browser.to_lowercase().as_str() {
        "brave" => BrowserKind::Brave,
        "chrome" => BrowserKind::Chrome,
        _ => {
            if let Some(info) = version_info {
                if let Some(ref b) = info.browser {
                    if b.starts_with("Brave") {
                        return BrowserKind::Brave;
                    }
                }
            }
            BrowserKind::Chrome
        }
    }
}
