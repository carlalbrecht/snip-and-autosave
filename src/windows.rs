//! Mostly-safe wrappers around various Win32 functions.
//!
//! For safety, most functions in this module check arguments, to ensure that
//! they are valid, before deferring to the Win32 function that they wrap. If
//! a wrapped function can fail, the result of the function call is checked, and
//! error details are extracted into a [`windows::Result`] `Err` variant.
//!
//! [`windows::Result`]: windows::Result

use crate::extensions::CStringExtensions;
use bindings::Windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, PSTR, WPARAM},
    Graphics::Gdi::BITMAPINFO,
    System::{
        Com::{CoInitializeEx, COINIT},
        Console::AttachConsole,
        DataExchange::{
            AddClipboardFormatListener, CloseClipboard, GetClipboardData,
            GetPriorityClipboardFormat, OpenClipboard,
        },
        LibraryLoader::GetModuleHandleA,
        ProcessStatus::K32GetProcessImageFileNameA,
        SystemServices::{CF_DIB, CLIPBOARD_FORMATS},
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
    UI::WindowsAndMessaging::{
        CreateWindowExA, DestroyMenu, DestroyWindow, DispatchMessageA, FindWindowA, GetMessageA,
        GetWindowThreadProcessId, LoadMenuA, PostQuitMessage, RegisterClassA, SendNotifyMessageA,
        TranslateMessage, CW_USEDEFAULT, HMENU, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSA,
        WNDPROC,
    },
};
use core::ptr;
use std::ffi::CString;
use std::time::Duration;
use std::{mem, thread};
use windows::{IntoParam, HRESULT};

/// The class name of the root message-only window used for clipboard events.
pub const CLASS_NAME: &str = "SnASWindow";

/// The name of the root message-only window.
pub const WINDOW_NAME: &str = "Snip & AutoSave";

/// Wraps a windows handle or resource, and closes it automatically, when it
/// goes out of scope.
pub struct AutoClose<T>
where
    T: Copy,
{
    value: T,
    close_fn: Box<dyn FnMut(T)>,
}

impl<T> AutoClose<T>
where
    T: Copy,
{
    /// Creates a new instance, wrapping a value that will be closed when it
    /// goes out of scope, by calling `close_fn` on it.
    pub fn new(value: T, close_fn: impl FnMut(T) + 'static) -> Self {
        Self {
            value,
            close_fn: Box::new(close_fn),
        }
    }

    /// Returns a copy of the handle owned by this instance. The returned handle
    /// must not be used once the `AutoClose` instance goes out of scope.
    pub fn value(&self) -> T {
        self.value
    }
}

impl<T> Drop for AutoClose<T>
where
    T: Copy,
{
    fn drop(&mut self) {
        (self.close_fn)(self.value);
    }
}

/// Attaches the current process to its parent process's console, if it has one.
///
/// Returns whether or not the console was attached. This will return `false` if
/// the program was started by double clicking on it in explorer, or via any
/// other graphical process.
pub fn attach_console() -> bool {
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;

    unsafe { AttachConsole(ATTACH_PARENT_PROCESS).0 != 0 }
}

/// Safe wrapper around [`CoInitializeEx`].
///
/// [`CoInitializeEx`]: CoInitializeEx
pub fn com_initialize(coinit: COINIT) -> windows::Result<()> {
    unsafe { CoInitializeEx(ptr::null_mut(), coinit) }
}

/// Safe wrapper around [`GetModuleHandleA`], which gets the module handle for
/// the current process.
///
/// [`GetModuleHandleA`]: GetModuleHandleA
pub fn get_instance() -> windows::Result<HINSTANCE> {
    unsafe {
        let handle = GetModuleHandleA(None);

        if handle.is_null() {
            Err(HRESULT::from_thread().into())
        } else {
            Ok(handle)
        }
    }
}

/// Safe wrapper around [`RegisterClassA`], which registers a new window class.
/// On success, returns the name of the class that was registered.
///
/// [`RegisterClassA`]: RegisterClassA
pub fn create_window_class(
    instance: HINSTANCE,
    class_name: &str,
    window_proc: Option<WNDPROC>,
) -> windows::Result<CString> {
    unsafe {
        let class_name = CString::new(class_name).unwrap();

        let atom = RegisterClassA(&WNDCLASSA {
            lpfnWndProc: window_proc,
            hInstance: instance,
            lpszClassName: class_name.as_pstr(),
            ..Default::default()
        });

        if atom == 0 {
            Err(HRESULT::from_thread().into())
        } else {
            Ok(class_name.into())
        }
    }
}

/// Safe wrapper around [`CreateWindowExA`], with most arguments pre-filled
/// specifically for creating message-only windows.
///
/// [`CreateWindowExA`]: CreateWindowExA
pub fn create_window(
    instance: HINSTANCE,
    class: &CString,
    window_name: &str,
) -> windows::Result<HWND> {
    unsafe {
        let window = CreateWindowExA(
            WINDOW_EX_STYLE(0),
            class.as_pstr(),
            window_name,
            WINDOW_STYLE(0),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            instance,
            ptr::null_mut(),
        );

        if window.is_null() {
            Err(HRESULT::from_thread().into())
        } else {
            Ok(window)
        }
    }
}

/// Safe wrapper around [`DestroyWindow`].
///
/// [`DestroyWindow`]: DestroyWindow
pub fn destroy_window(window: HWND) {
    unsafe {
        DestroyWindow(window);
    }
}

/// Safe wrapper around [`FindWindowA`], which returns the [`HWND`] of a window
/// with the specified class and window name, if one exists.
///
/// On failure, this function returns `None`.
///
/// [`FindWindowA`]: FindWindowA
/// [`HWND`]: HWND
pub fn find_window(class_name: &str, window_name: &str) -> Option<HWND> {
    let window = unsafe { FindWindowA(class_name, window_name) };

    if window.is_null() {
        None
    } else {
        Some(window)
    }
}

/// Safe wrapper around [`SendNotifyMessageA`].
///
/// [`SendNotifyMessageA`]: SendNotifyMessageA
pub fn send_notify_message(
    window: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> windows::Result<()> {
    if unsafe { SendNotifyMessageA(window, message, w_param, l_param).0 != 0 } {
        Ok(())
    } else {
        Err(HRESULT::from_thread().into())
    }
}

/// Safe wrapper around [`AddClipboardFormatListener`], which registers a
/// [`HWND`] for receiving [`WM_CLIPBOARDUPDATE`] messages.
///
/// [`AddClipboardFormatListener`]: AddClipboardFormatListener
/// [`HWND`]: HWND
/// [`WM_CLIPBOARDUPDATE`]: bindings::Windows::Win32::UI::WindowsAndMessaging::WM_CLIPBOARDUPDATE
pub fn add_clipboard_listener(window: HWND) -> windows::Result<()> {
    unsafe {
        match AddClipboardFormatListener(window).0 {
            0 => Err(HRESULT::from_thread().into()),
            _ => Ok(()),
        }
    }
}

/// Safe wrapper around [`GetWindowThreadProcessId`], which obtains the process
/// and thread IDs of the owner of a [`HWND`].
///
/// [`GetWindowThreadProcessId`]: GetWindowThreadProcessId
/// [`HWND`]: HWND
pub fn get_window_thread_and_process_id(window: HWND) -> (u32, u32) {
    let mut process_id: u32 = 0;
    let thread_id = unsafe { GetWindowThreadProcessId(window, &mut process_id) };

    (process_id, thread_id)
}

/// Safe wrapper around [`OpenProcess`], which opens a handle to a process, with
/// [`PROCESS_QUERY_LIMITED_INFORMATION`] access rights. The returned handle is
/// closed automatically, when it goes out of scope.
///
/// [`OpenProcess`]: OpenProcess
/// [`PROCESS_QUERY_LIMITED_INFORMATION`]: PROCESS_QUERY_LIMITED_INFORMATION
pub fn open_process(process_id: u32) -> windows::Result<AutoClose<HANDLE>> {
    unsafe {
        let process_handle = AutoClose::new(
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id),
            |p| {
                CloseHandle(p);
            },
        );

        if process_handle.value().is_null() {
            Err(HRESULT::from_thread().into())
        } else {
            Ok(process_handle)
        }
    }
}

/// Safe wrapper around [`K32GetProcessImageFileNameA`], which gets the path to
/// the process image (i.e. the executable file for the process).
///
/// Note that the returned path is actually an NT path, not a Windows path.
/// Therefore, rather than getting a result like
/// `"C:\Windows\System32\svchost.exe"`, you'll get a result more like
/// `"\Device\HarddiskVolume1\Windows\System32\svchost.exe"`.
///
/// The image name is copied into a fixed length C-string buffer before
/// conversion to a Rust [`String`]. Currently, this function can handle image
/// names up to 256-bytes in length.
///
/// [`K32GetProcessImageFileNameA`]: K32GetProcessImageFileNameA
/// [`String`]: String
pub fn get_process_image_file_name(process_handle: HANDLE) -> windows::Result<String> {
    const FILENAME_MAX_BYTES: usize = 256;

    let mut filename_raw = vec![0; FILENAME_MAX_BYTES + 1];

    let filename_length = unsafe {
        K32GetProcessImageFileNameA(
            process_handle,
            PSTR(filename_raw.as_mut_ptr()),
            FILENAME_MAX_BYTES as u32,
        )
    };

    if filename_length == 0 {
        Err(HRESULT::from_thread().into())
    } else {
        filename_raw.truncate(filename_length as usize);

        Ok(String::from_utf8(filename_raw)
            .expect("Invalid UTF-8 returned by GetProcessImageFileNameA"))
    }
}

/// [`OpenClipboard`] wrapper for [`open_clipboard`], which performs the actual
/// call to [`OpenClipboard`], for a single attempt at opening the clipboard.
///
/// [`OpenClipboard`]: OpenClipboard
/// [`open_clipboard`]: open_clipboard
fn open_clipboard_inner(window: Option<HWND>) -> windows::Result<AutoClose<()>> {
    if unsafe { OpenClipboard(window.unwrap_or(HWND(0))).0 != 0 } {
        Ok(AutoClose::new((), |_| unsafe {
            CloseClipboard();
        }))
    } else {
        Err(HRESULT::from_thread().into())
    }
}

/// Safe wrapper around [`OpenClipboard`].
///
/// The returned [`AutoClose`] instance must be kept alive for as long as access
/// to the clipboard data is needed. When this instance is dropped, the
/// clipboard is closed, allowing other programs to access it.
///
/// As it is possible that another process is in the middle of accessing the
/// clipboard when this function is called, it will retry up to 5 times, 50
/// milliseconds apart, to open the clipboard.
///
/// [`OpenClipboard`]: OpenClipboard
/// [`AutoClose`]: AutoClose
pub fn open_clipboard(window: Option<HWND>) -> windows::Result<AutoClose<()>> {
    const RETRY_INTERVAL: Duration = Duration::from_millis(50);

    let mut result: windows::Result<AutoClose<()>> = open_clipboard_inner(window.clone());

    for _ in 0..5 {
        if result.is_ok() {
            break;
        }

        thread::sleep(RETRY_INTERVAL);

        result = open_clipboard_inner(window.clone());
    }

    result
}

/// Safe wrapper around [`GetPriorityClipboardFormat`], which returns the first
/// clipboard format in `formats` that the current data on the clipboard is
/// either in, or can be converted to by the operating system.
///
/// If the clipboard data cannot be converted (e.g. the clipboard contains text,
/// whilst `formats` is asking for one or more bitmap formats), `None` is
/// returned.
///
/// [`GetPriorityClipboardFormat`]: GetPriorityClipboardFormat
pub fn get_priority_clipboard_format(formats: &[CLIPBOARD_FORMATS]) -> Option<CLIPBOARD_FORMATS> {
    let format =
        unsafe { GetPriorityClipboardFormat(formats.as_ptr() as *mut u32, formats.len() as i32) };

    if format <= 0 {
        None
    } else {
        Some(CLIPBOARD_FORMATS(format as u32))
    }
}

/// Unsafe wrapper around [`GetClipboardData`], which retrieves the clipboard
/// data in the specified `format`, then applies a C-style reinterpret cast on
/// the raw handle returned by [`GetClipboardData`], in order to return data in
/// the format specified by `format`.
///
/// Returns an `Err` result when the clipboard data is not available in the
/// requested `format` (i.e. [`get_priority_clipboard_format`] would return
/// `None` for the requested `format`).
///
/// # Safety
///
/// `T` must match the type of data that the handle returned by
/// [`GetClipboardData`] points to, for the specified `format`. For example,
/// if the [`CF_DIB`] clipboard format is requested, `T` must equal
/// [`BITMAPINFO`], so that a pointer to a device-independent bitmap is
/// returned.
///
/// A list of standard bitmap `format`s, and the data type they return, is
/// available [here].
///
/// [`GetClipboardData`]: GetClipboardData
/// [`get_priority_clipboard_format`]: get_priority_clipboard_format
/// [`CF_DIB`]: CF_DIB
/// [`BITMAPINFO`]: BITMAPINFO
/// [here]: https://docs.microsoft.com/en-us/windows/win32/dataxchg/standard-clipboard-formats
pub unsafe fn get_clipboard_data<T>(format: CLIPBOARD_FORMATS) -> windows::Result<*const T> {
    let handle = GetClipboardData(format.0);

    if handle.is_null() {
        Err(HRESULT::from_thread().into())
    } else {
        Ok(mem::transmute::<_, *const T>(handle))
    }
}

/// Retrieves the current clipboard contents, as a [`CF_DIB`]
/// (i.e., a device-independent bitmap), via [`get_clipboard_data`].
///
/// [`CF_DIB`]: CF_DIB
/// [`get_clipboard_data`]: get_clipboard_data
pub fn get_clipboard_dib() -> windows::Result<*const BITMAPINFO> {
    unsafe { get_clipboard_data::<BITMAPINFO>(CF_DIB) }
}

/// Unsafe wrapper around [`LoadMenuA`], which loads a menu from a Windows
/// resource file, that has been compiled into the executable file.
/// 
/// # Safety
/// 
/// `menu_name` must either point to a valid C-string, or have its pointer value
/// set to the resource ID of a `MENU` structure.
/// 
/// [`LoadMenuA`]: LoadMenuA
pub unsafe fn load_menu<'a>(
    instance: HINSTANCE,
    menu_name: impl IntoParam<'a, PSTR>,
) -> AutoClose<HMENU> {
    let menu = LoadMenuA(instance, menu_name);

    AutoClose::new(menu, |m| {
        DestroyMenu(m);
    })
}

/// Safe wrapper around [`PostQuitMessage`], which posts a [`WM_QUIT`] message
/// to the current thread's message queue.
///
/// This should be called when a window / thread receives a [`WM_DESTROY`]
/// message, and the application should exit.
///
/// [`PostQuitMessage`]: PostQuitMessage
/// [`WM_QUIT`]: bindings::Windows::Win32::UI::WindowsAndMessaging::WM_QUIT
/// [`WM_DESTROY`]: bindings::Windows::Win32::UI::WindowsAndMessaging::WM_DESTROY
pub fn post_quit_message(exit_code: i32) {
    unsafe { PostQuitMessage(exit_code) };
}

/// Starts a blocking message loop for the specified `window`. This will block
/// indefinitely, if a specific `window` is specified. Otherwise, if `HWND(0)`
/// is passed in, the message loop listens for all messages on the current
/// thread, and will terminate once a [`WM_QUIT`] message is received (e.g. from
/// [`post_quit_message`]).
///
/// [`WM_QUIT`]: bindings::Windows::Win32::UI::WindowsAndMessaging::WM_QUIT
/// [`post_quit_message`]: post_quit_message
pub fn message_loop(window: HWND) {
    let mut message = MSG::default();

    unsafe {
        while GetMessageA(&mut message, window, 0, 0).0 != 0 {
            TranslateMessage(&message);
            DispatchMessageA(&message);
        }
    }
}
