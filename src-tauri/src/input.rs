//! Windows input plumbing: track the last foreign foreground window, grab text from
//! the focused field via the clipboard, and paste the result back.

use std::thread::sleep;
use std::time::{Duration, Instant};

pub fn set_clipboard(text: &str) -> Result<(), String> {
    for _ in 0..5 {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if cb.set_text(text.to_owned()).is_ok() {
                return Ok(());
            }
        }
        sleep(Duration::from_millis(30));
    }
    Err("Could not write to clipboard".into())
}

fn get_clipboard() -> Result<String, String> {
    let mut last_err = String::from("clipboard unavailable");
    for _ in 0..5 {
        match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
            Ok(t) => return Ok(t),
            Err(e) => last_err = e.to_string(),
        }
        sleep(Duration::from_millis(30));
    }
    Err(format!("Could not read clipboard: {last_err}"))
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU,
        VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
    };

    static LAST_FOREIGN: AtomicIsize = AtomicIsize::new(0);

    fn is_ours(h: HWND) -> bool {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, &mut pid) };
        pid == unsafe { GetCurrentProcessId() }
    }

    /// Called periodically: remember the last window that isn't ours.
    pub fn track_foreground() {
        let h = unsafe { GetForegroundWindow() };
        if !h.is_null() && !is_ours(h) {
            LAST_FOREIGN.store(h as isize, Ordering::Relaxed);
        }
    }

    /// If one of our windows grabbed focus (bubble click), hand it back to the user's app.
    pub fn restore_foreground() {
        let fg = unsafe { GetForegroundWindow() };
        if !fg.is_null() && !is_ours(fg) {
            return; // user's app already has focus (hotkey path)
        }
        let last = LAST_FOREIGN.load(Ordering::Relaxed) as HWND;
        if !last.is_null() && unsafe { IsWindow(last) } != 0 {
            unsafe { SetForegroundWindow(last) };
            sleep(Duration::from_millis(120));
        }
    }

    fn key(vk: u16, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
    }

    const MODS: [u16; 11] = [
        VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN, VK_LSHIFT, VK_RSHIFT, VK_LCONTROL,
        VK_RCONTROL, VK_LMENU, VK_RMENU,
    ];

    fn held(vk: u16) -> bool {
        (unsafe { GetAsyncKeyState(vk as i32) } as u16) & 0x8000 != 0
    }

    /// Hotkey modifiers (e.g. Ctrl+Shift) still held would turn our Ctrl+C into Ctrl+Shift+C.
    pub fn wait_modifiers_released() {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(1200) {
            if !MODS.iter().any(|&v| held(v)) {
                return;
            }
            sleep(Duration::from_millis(15));
        }
        let ups: Vec<INPUT> = MODS.iter().filter(|&&v| held(v)).map(|&v| key(v, true)).collect();
        if !ups.is_empty() {
            send(&ups);
            sleep(Duration::from_millis(30));
        }
    }

    /// Ctrl+<letter>
    pub fn ctrl(letter: u8) {
        let vk = letter.to_ascii_uppercase() as u16;
        send(&[key(VK_CONTROL, false), key(vk, false), key(vk, true), key(VK_CONTROL, true)]);
    }

    pub fn clip_seq() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }
}

#[cfg(windows)]
pub use win::track_foreground;

#[cfg(windows)]
fn wait_clip_change(seq0: u32, ms: u64) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {
        if win::clip_seq() != seq0 {
            sleep(Duration::from_millis(25)); // let the source app finish writing
            return true;
        }
        sleep(Duration::from_millis(10));
    }
    false
}

/// Copy the current selection, or (if nothing selected) the whole focused field.
#[cfg(windows)]
pub fn capture() -> Result<String, String> {
    win::wait_modifiers_released();
    win::restore_foreground();

    let seq = win::clip_seq();
    win::ctrl(b'c');
    if !wait_clip_change(seq, 250) {
        win::ctrl(b'a');
        sleep(Duration::from_millis(40));
        let seq = win::clip_seq();
        win::ctrl(b'c');
        if !wait_clip_change(seq, 500) {
            return Err("No text found. Click inside a text box first.".into());
        }
    }
    get_clipboard()
}

/// Put text on the clipboard and paste it over the (still active) selection.
#[cfg(windows)]
pub fn paste(text: &str) -> Result<(), String> {
    set_clipboard(text)?;
    win::wait_modifiers_released();
    win::restore_foreground();
    sleep(Duration::from_millis(40));
    win::ctrl(b'v');
    Ok(())
}

#[cfg(not(windows))]
pub fn track_foreground() {}

#[cfg(not(windows))]
pub fn capture() -> Result<String, String> {
    let _ = Instant::now();
    get_clipboard()
}

#[cfg(not(windows))]
pub fn paste(text: &str) -> Result<(), String> {
    set_clipboard(text)
}
