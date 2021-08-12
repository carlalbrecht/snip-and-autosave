//! Notification area icon management functions.
//!
//! This module relies on several private, `unsafe` Win32 function wrappers,
//! which aren't exposed via [`windows`].
//!
//! [`windows`]: crate::windows

use crate::extensions::CStringExtensions;
use crate::settings::Settings;
use crate::windows::{
    create_link, get_instance, get_known_folder_path, load_menu, send_notify_message,
};
use bindings::Windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, PSTR, WPARAM},
    System::SystemServices::CHAR,
    UI::{
        Controls::{LoadIconMetric, LIM_SMALL, WM_CONTEXTMENU},
        Shell::{
            FOLDERID_Startup, ShellExecuteA, Shell_NotifyIconA, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP,
            NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NOTIFYICONDATAA, NOTIFYICONDATAA_0,
            NOTIFYICON_VERSION_4, NOTIFY_ICON_DATA_FLAGS, NOTIFY_ICON_MESSAGE,
        },
        WindowsAndMessaging::{
            CheckMenuItem, GetSubMenu, GetSystemMetrics, SetForegroundWindow, TrackPopupMenuEx,
            HICON, MF_CHECKED, MF_UNCHECKED, SM_MENUDROPALIGNMENT, SW_SHOWNORMAL, TPM_LEFTALIGN,
            TPM_RIGHTALIGN, TPM_RIGHTBUTTON, WM_APP, WM_CLOSE,
        },
    },
};
use rfd::FileDialog;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{env, mem, ptr, thread};
use windows::{Guid, HRESULT};

// Specified in `build.rs:compile_windows_resources`
static ICON_IDENTIFIER: &str = "IDI_APPLICATION_ICON";

// Ampersands in notification area tooltips require double-escaping:
// https://stackoverflow.com/a/10279419/13166644
static ICON_TOOLTIP: &str = "Snip &&& AutoSave";

const IDM_EXIT: usize = 121;
const IDM_SET_LOCATION: usize = 122;
const IDM_OPEN_LOCATION: usize = 123;
const IDM_START_AUTOMATICALLY: usize = 124;

/// The message ID of notification area icon messages.
pub const WMAPP_NOTIFYCALLBACK: u32 = WM_APP + 1;

/// Creates a notification area icon for this application.
///
/// `window` specifies the window that owns the icon. Notification area icon
/// messages ([`WMAPP_NOTIFYCALLBACK`]) will be sent to the `wndProc` function
/// for this window.
///
/// If an icon for this program already exists, it is removed, before a new one
/// is created. This only happens if the application was forcefully terminated,
/// preventing its clean-up routines from removing the icon in the last run.
///
/// [`WMAPP_NOTIFYCALLBACK`]: WMAPP_NOTIFYCALLBACK
pub fn create_icon(window: HWND) -> windows::Result<()> {
    // If the icon still exists from a previous run (i.e. the program was forcefully terminated,
    // thus preventing it from removing the icon before closing), it will prevent us from creating
    // a new icon. Therefore, we remove it, if it exists.
    let _ = remove_icon(window);

    let mut tooltip = [CHAR(0); 128];

    tooltip[..ICON_TOOLTIP.len()]
        .copy_from_slice(unsafe { mem::transmute::<_, &[CHAR]>(ICON_TOOLTIP.as_bytes()) });
    tooltip[127] = CHAR(0);

    let mut icon_data = NOTIFYICONDATAA {
        hWnd: window,
        uID: 0,
        uFlags: NIF_ICON | NIF_TIP | NIF_MESSAGE | NIF_SHOWTIP,
        uCallbackMessage: WMAPP_NOTIFYCALLBACK,
        hIcon: unsafe {
            LoadIconMetric(get_instance().unwrap(), ICON_IDENTIFIER, LIM_SMALL).unwrap()
        },
        szTip: tooltip,
        Anonymous: NOTIFYICONDATAA_0 {
            uVersion: NOTIFYICON_VERSION_4,
        },
        ..default_notify_icon_data()
    };

    shell_notify_icon(NIM_ADD, &mut icon_data)?;
    shell_notify_icon(NIM_SETVERSION, &mut icon_data)?;

    Ok(())
}

/// Removes the notification area icon for this application.
pub fn remove_icon(window: HWND) -> windows::Result<()> {
    let mut icon_data = NOTIFYICONDATAA {
        hWnd: window,
        uID: 0,
        ..default_notify_icon_data()
    };

    shell_notify_icon(NIM_DELETE, &mut icon_data)?;

    Ok(())
}

/// Message handler for notification area icon messages.
///
/// This should be called from the `wndProc` function for the [`HWND`] that the
/// notification area icon was created under.
///
/// [`HWND`]: HWND
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

/// [`WM_COMMAND`] processor, which handles commands related to the notification
/// area icon (e.g. the icon's context menu entries).
///
/// [`WM_COMMAND`]: bindings::Windows::Win32::UI::WindowsAndMessaging::WM_COMMAND
pub fn on_command(window: HWND, command: usize) -> Option<LRESULT> {
    match command {
        IDM_EXIT => {
            send_notify_message(window, WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
            Some(LRESULT(0))
        }
        IDM_SET_LOCATION => {
            set_screenshot_dir();
            Some(LRESULT(0))
        }
        IDM_OPEN_LOCATION => {
            explore_screenshot_dir(window).unwrap();
            Some(LRESULT(0))
        }
        IDM_START_AUTOMATICALLY => {
            toggle_auto_start().unwrap();
            Some(LRESULT(0))
        }
        _ => None,
    }
}

/// Safe wrapper around [`Shell_NotifyIconA`].
///
/// [`Shell_NotifyIconA`]: Shell_NotifyIconA
fn shell_notify_icon(
    message: NOTIFY_ICON_MESSAGE,
    data: &mut NOTIFYICONDATAA,
) -> windows::Result<()> {
    if unsafe { Shell_NotifyIconA(message, data).0 != 0 } {
        Ok(())
    } else {
        Err(HRESULT::from_thread().into())
    }
}

/// Returns a default (zeroed) [`NOTIFYICONDATAA`] instance. the `cbSize` field
/// is initialised properly, to the size of the [`NOTIFYICONDATAA`] struct.
///
/// This essentially functions like a [`Default`] implementation.
///
/// [`NOTIFYICONDATAA`]: NOTIFYICONDATAA
/// [`Default`]: Default
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

/// Shows the context menu for the notification area icon.
///
/// # Arguments
///
/// * `window`  - The window that owns the notification area icon.
/// * `click_x` - The mouse X position of the right click.
/// * `click_y` - The mouse Y position of the right click.
fn show_context_menu(window: HWND, (click_x, click_y): (usize, usize)) {
    let mut auto_start = false;
    Settings::read(|s| auto_start = s.program.auto_start);

    unsafe {
        let menu = load_menu(get_instance().unwrap(), PSTR(200 as *mut u8));
        let submenu = GetSubMenu(menu.value(), 0);

        CheckMenuItem(
            menu.value(),
            IDM_START_AUTOMATICALLY as u32,
            if auto_start {
                MF_CHECKED.0
            } else {
                MF_UNCHECKED.0
            },
        );

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

/// Opens a folder select dialog, to select the directory to save captured
/// screenshots to.
///
/// If the user accepts a directory in the dialog, it is written to the global
/// application [`Settings`].
///
/// This function is a no-op if a folder select dialog is already open.
///
/// [`Settings`]: Settings
fn set_screenshot_dir() {
    static IS_BROWSING: AtomicBool = AtomicBool::new(false);

    thread::spawn(|| {
        if IS_BROWSING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Acquire)
            .is_err()
        {
            // We already have a file dialog open
            return;
        }

        let mut screenshot_path = PathBuf::new();

        Settings::read(|s| {
            screenshot_path = find_existing_parent(&s.paths.screenshots);
        });

        if let Some(new_path) = FileDialog::new()
            .set_directory(screenshot_path)
            .pick_folder()
        {
            Settings::write(|s| s.paths.screenshots = new_path);
        }

        IS_BROWSING.store(false, Ordering::SeqCst);
    });
}

/// Opens an explorer window to the current screenshot output directory.
fn explore_screenshot_dir(window: HWND) -> windows::Result<()> {
    let operation = CString::new("explore").unwrap();
    let mut folder: CString = CString::new("").unwrap();

    Settings::read(|s| {
        // This isn't optimal, but, rather than failing if the path doesn't exist, we just open an
        // explorer window to a parent folder in the path that does exist
        let parent = find_existing_parent(&s.paths.screenshots);

        folder = CString::new(parent.to_str().unwrap()).unwrap();
    });

    if unsafe {
        ShellExecuteA(
            window,
            operation.as_pstr(),
            folder.as_pstr(),
            PSTR(ptr::null_mut()),
            PSTR(ptr::null_mut()),
            SW_SHOWNORMAL.0 as i32,
        )
        .0 <= 32
    } {
        Err(HRESULT::from_thread().into())
    } else {
        Ok(())
    }
}

/// Finds the first existing path, starting from a child path. This allows
/// callers to handle navigation to directories that have since been deleted by
/// the user.
fn find_existing_parent(path: &Path) -> PathBuf {
    let mut current_path = path;

    // Find first parent directory that exists (in case the user deleted the currently
    // configured screenshot directory). Otherwise, if we attempt to open a dialog starting
    // from a directory that doesn't exist, the dialog simply does not appear.
    while !current_path.exists() {
        if let Some(parent) = current_path.parent() {
            current_path = parent;
        } else {
            break;
        }
    }

    current_path.into()
}

/// Adds / removes a shortcut to this program from the user's start-up folder,
/// depending on their current auto-start setting.
fn toggle_auto_start() -> windows::Result<()> {
    let mut auto_start = false;
    Settings::read(|s| auto_start = s.program.auto_start);

    let mut startup_path = get_known_folder_path(FOLDERID_Startup)?;
    startup_path.push("Snip & AutoSave.lnk");

    if auto_start {
        // Remove shortcut from start-up folder
        let _ = std::fs::remove_file(startup_path);

        Settings::write(|s| s.program.auto_start = false);
    } else {
        // Add shortcut to start-up folder
        create_link(
            &startup_path,
            dunce::simplified(&env::current_exe().unwrap()),
        )?;

        Settings::write(|s| s.program.auto_start = true);
    }

    Ok(())
}
