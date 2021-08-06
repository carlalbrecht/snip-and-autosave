use crate::windows::get_instance;
use bindings::Windows::Win32::{
    Foundation::{HWND, PWSTR},
    System::SystemServices::CHAR,
    UI::{
        Controls::{LoadIconMetric, LIM_SMALL},
        Shell::{
            Shell_NotifyIconA, NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
            NIM_SETVERSION, NOTIFYICONDATAA, NOTIFYICONDATAA_0, NOTIFYICON_VERSION_4,
        },
        WindowsAndMessaging::HICON,
    },
};
use std::mem;
use windows::{Guid, HRESULT};

// Specified in `build.rs:compile_windows_resources`
static ICON_IDENTIFIER: &str = "IDI_APPLICATION_ICON";

// Ampersands in notification area tooltips require double-escaping:
// https://stackoverflow.com/a/10279419/13166644
static ICON_TOOLTIP: &str = "Snip &&& AutoSave";

// 02b72d97-c85d-463b-804e-af47dcabc45a
const ICON_GUID: Guid = Guid::from_values(
    0x02b72d97,
    0xc85d,
    0x463b,
    [0x80, 0x4e, 0xaf, 0x47, 0xdc, 0xab, 0xc4, 0x5a],
);

pub fn create_icon(window: HWND) {
    let mut tooltip = [CHAR(0); 128];

    tooltip[..ICON_TOOLTIP.len()]
        .copy_from_slice(unsafe { mem::transmute::<_, &[CHAR]>(ICON_TOOLTIP.as_bytes()) });
    tooltip[127] = CHAR(0);

    let mut icon_data = NOTIFYICONDATAA {
        cbSize: mem::size_of::<NOTIFYICONDATAA>() as u32,
        hWnd: window,
        uID: 0,
        uFlags: NIF_ICON | NIF_TIP | NIF_MESSAGE | NIF_SHOWTIP | NIF_GUID,
        uCallbackMessage: 0, // TODO
        hIcon: unsafe {
            LoadIconMetric(get_instance().unwrap(), ICON_IDENTIFIER, LIM_SMALL).unwrap()
        },
        szTip: tooltip,
        dwState: 0,
        dwStateMask: 0,
        szInfo: [CHAR(0); 256],
        Anonymous: NOTIFYICONDATAA_0 {
            uVersion: NOTIFYICON_VERSION_4,
        },
        szInfoTitle: [CHAR(0); 64],
        dwInfoFlags: 0,
        guidItem: ICON_GUID,
        hBalloonIcon: HICON(0),
    };

    unsafe {
        Shell_NotifyIconA(NIM_ADD, &mut icon_data);
        Shell_NotifyIconA(NIM_SETVERSION, &mut icon_data);
    }
}

pub fn remove_icon(window: HWND) {}
