use bindings::Windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, PSTR},
    System::{
        DataExchange::{AddClipboardFormatListener, GetPriorityClipboardFormat},
        LibraryLoader::GetModuleHandleA,
        ProcessStatus::K32GetProcessImageFileNameA,
        SystemServices::CLIPBOARD_FORMATS,
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
    UI::WindowsAndMessaging::{
        CreateWindowExA, DispatchMessageA, GetMessageA, GetWindowThreadProcessId, RegisterClassA,
        TranslateMessage, CW_USEDEFAULT, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSA, WNDPROC,
    },
};
use core::ptr;
use std::ffi::CString;
use windows::HRESULT;

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
    window_proc: Option<WNDPROC>,
) -> windows::Result<CString> {
    unsafe {
        let class_name = CString::new("Window").expect("CString::new failed");

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

pub fn create_window(instance: HINSTANCE, class: &CString) -> windows::Result<HWND> {
    unsafe {
        let window = CreateWindowExA(
            WINDOW_EX_STYLE(0),
            PSTR(class.as_ptr() as *mut u8),
            "Snip & AutoSave",
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

pub fn get_priority_clipboard_format(formats: &[CLIPBOARD_FORMATS]) -> Option<CLIPBOARD_FORMATS> {
    let format =
        unsafe { GetPriorityClipboardFormat(formats.as_ptr() as *mut u32, formats.len() as i32) };

    if format <= 0 {
        None
    } else {
        Some(CLIPBOARD_FORMATS(format as u32))
    }
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
