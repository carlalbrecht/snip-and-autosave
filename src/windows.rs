use bindings::Windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, PSTR},
    Graphics::Gdi::BITMAPINFO,
    System::{
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
        CreateWindowExA, DispatchMessageA, FindWindowA, GetMessageA, GetWindowThreadProcessId,
        PostQuitMessage, RegisterClassA, TranslateMessage, CW_USEDEFAULT, MSG, WINDOW_EX_STYLE,
        WINDOW_STYLE, WNDCLASSA, WNDPROC,
    },
};
use core::ptr;
use std::ffi::CString;
use std::time::Duration;
use std::{mem, thread};
use windows::HRESULT;

pub const CLASS_NAME: &str = "SnASWindow";
pub const WINDOW_NAME: &str = "Snip & AutoSave";

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
    pub fn new(value: T, close_fn: impl FnMut(T) + 'static) -> Self {
        Self {
            value,
            close_fn: Box::new(close_fn),
        }
    }

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

pub fn create_window_class(
    instance: HINSTANCE,
    class_name: &str,
    window_proc: Option<WNDPROC>,
) -> windows::Result<CString> {
    unsafe {
        let class_name = CString::new(class_name).expect("CString::new failed");

        let atom = RegisterClassA(&WNDCLASSA {
            lpfnWndProc: window_proc,
            hInstance: instance,
            lpszClassName: PSTR(class_name.as_ptr() as *mut u8),
            ..Default::default()
        });

        if atom == 0 {
            Err(HRESULT::from_thread().into())
        } else {
            Ok(class_name.into())
        }
    }
}

pub fn create_window(
    instance: HINSTANCE,
    class: &CString,
    window_name: &str,
) -> windows::Result<HWND> {
    unsafe {
        let window = CreateWindowExA(
            WINDOW_EX_STYLE(0),
            PSTR(class.as_ptr() as *mut u8),
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

pub fn find_window(class_name: &str, window_name: &str) -> Option<HWND> {
    let window = unsafe { FindWindowA(class_name, window_name) };

    if window.is_null() {
        None
    } else {
        Some(window)
    }
}

pub fn add_clipboard_listener(window: HWND) -> windows::Result<()> {
    unsafe {
        match AddClipboardFormatListener(window).0 {
            0 => Err(HRESULT::from_thread().into()),
            _ => Ok(()),
        }
    }
}

pub fn get_window_thread_and_process_id(window: HWND) -> (u32, u32) {
    let mut process_id: u32 = 0;
    let thread_id = unsafe { GetWindowThreadProcessId(window, &mut process_id) };

    (process_id, thread_id)
}

pub fn open_process(process: u32) -> windows::Result<AutoClose<HANDLE>> {
    unsafe {
        let process_handle = AutoClose::new(
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process),
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

fn open_clipboard_inner(window: Option<HWND>) -> windows::Result<AutoClose<()>> {
    if unsafe { OpenClipboard(window.unwrap_or(HWND(0))).0 != 0 } {
        Ok(AutoClose::new((), |_| unsafe {
            CloseClipboard();
        }))
    } else {
        Err(HRESULT::from_thread().into())
    }
}

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

pub fn get_priority_clipboard_format(formats: &[CLIPBOARD_FORMATS]) -> Option<CLIPBOARD_FORMATS> {
    let format =
        unsafe { GetPriorityClipboardFormat(formats.as_ptr() as *mut u32, formats.len() as i32) };

    if format <= 0 {
        None
    } else {
        Some(CLIPBOARD_FORMATS(format as u32))
    }
}

pub unsafe fn get_clipboard_data<T>(format: CLIPBOARD_FORMATS) -> windows::Result<*const T> {
    let handle = GetClipboardData(format.0);

    if handle.is_null() {
        Err(HRESULT::from_thread().into())
    } else {
        Ok(mem::transmute::<_, *const T>(handle))
    }
}

pub fn get_clipboard_dib() -> windows::Result<*const BITMAPINFO> {
    unsafe { get_clipboard_data::<BITMAPINFO>(CF_DIB) }
}

pub fn post_quit_message(exit_code: i32) {
    unsafe { PostQuitMessage(exit_code) };
}

pub fn message_loop(window: HWND) {
    let mut message = MSG::default();

    unsafe {
        while GetMessageA(&mut message, window, 0, 0).0 != 0 {
            TranslateMessage(&message);
            DispatchMessageA(&message);
        }
    }
}
