use crate::windows::{get_instance, load_menu};
use bindings::Windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, PSTR, WPARAM},
    System::SystemServices::CHAR,
    UI::{
        Controls::{LoadIconMetric, LIM_SMALL, WM_CONTEXTMENU},
        Shell::{
            Shell_NotifyIconA, NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
            NIM_DELETE, NIM_SETVERSION, NOTIFYICONDATAA, NOTIFYICONDATAA_0, NOTIFYICON_VERSION_4,
            NOTIFY_ICON_DATA_FLAGS,
        },
        WindowsAndMessaging::{
            GetSubMenu, GetSystemMetrics, SetForegroundWindow, TrackPopupMenuEx, HICON,
            SM_MENUDROPALIGNMENT, TPM_LEFTALIGN, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, WM_APP,
        },
    },
};
use std::{mem, ptr};
use windows::Guid;

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

pub const WMAPP_NOTIFYCALLBACK: u32 = WM_APP + 1;

pub fn create_icon(window: HWND) {
    // If the icon still exists from a previous run (i.e. the program was forcefully terminated,
    // thus preventing it from removing the icon before closing), it will prevent us from creating
    // a new icon. Therefore, we remove it, if it exists.
    remove_icon();

    let mut tooltip = [CHAR(0); 128];

    tooltip[..ICON_TOOLTIP.len()]
        .copy_from_slice(unsafe { mem::transmute::<_, &[CHAR]>(ICON_TOOLTIP.as_bytes()) });
    tooltip[127] = CHAR(0);

    let mut icon_data = NOTIFYICONDATAA {
        hWnd: window,
        uFlags: NIF_ICON | NIF_TIP | NIF_MESSAGE | NIF_SHOWTIP | NIF_GUID,
        uCallbackMessage: WMAPP_NOTIFYCALLBACK,
        hIcon: unsafe {
            LoadIconMetric(get_instance().unwrap(), ICON_IDENTIFIER, LIM_SMALL).unwrap()
        },
        szTip: tooltip,
        Anonymous: NOTIFYICONDATAA_0 {
            uVersion: NOTIFYICON_VERSION_4,
        },
        guidItem: ICON_GUID,
        ..default_notify_icon_data()
    };

    unsafe {
        Shell_NotifyIconA(NIM_ADD, &mut icon_data);
        Shell_NotifyIconA(NIM_SETVERSION, &mut icon_data);
    }
}

pub fn remove_icon() {
    let mut icon_data = NOTIFYICONDATAA {
        uFlags: NIF_GUID,
        guidItem: ICON_GUID,
        ..default_notify_icon_data()
    };

    unsafe {
        Shell_NotifyIconA(NIM_DELETE, &mut icon_data);
    }
}

//noinspection RsUnreachablePatterns
pub fn notify_callback(window: HWND, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    match (l_param.0 & 0xFFFF) as u32 {
        WM_CONTEXTMENU => {
            let click_location = (w_param.0 & 0xFFFF, (w_param.0 >> 16) & 0xFFFF);
            show_context_menu(window, click_location);

            LRESULT(0)
        }
        _ => LRESULT(0),
    }
}

fn default_notify_icon_data() -> NOTIFYICONDATAA {
    NOTIFYICONDATAA {
        cbSize: mem::size_of::<NOTIFYICONDATAA>() as u32,
        hWnd: HWND(0),
        uID: 0,
        uFlags: NOTIFY_ICON_DATA_FLAGS(0),
        uCallbackMessage: 0,
        hIcon: HICON(0),
        szTip: [CHAR(0); 128],
        dwState: 0,
        dwStateMask: 0,
        szInfo: [CHAR(0); 256],
        Anonymous: NOTIFYICONDATAA_0 { uVersion: 0 },
        szInfoTitle: [CHAR(0); 64],
        dwInfoFlags: 0,
        guidItem: Guid::zeroed(),
        hBalloonIcon: HICON(0),
    }
}

fn show_context_menu(window: HWND, (click_x, click_y): (usize, usize)) {
    unsafe {
        let menu = load_menu(get_instance().unwrap(), PSTR(200 as *mut u8));
        let submenu = GetSubMenu(menu.value(), 0);

        SetForegroundWindow(window);

        let mut popup_flags = TPM_RIGHTBUTTON;

        if GetSystemMetrics(SM_MENUDROPALIGNMENT) != 0 {
            popup_flags |= TPM_RIGHTALIGN;
        } else {
            popup_flags |= TPM_LEFTALIGN;
        }

        TrackPopupMenuEx(
            submenu,
            popup_flags.0,
            click_x as i32,
            click_y as i32,
            window,
            ptr::null_mut(),
        );
    }
}
