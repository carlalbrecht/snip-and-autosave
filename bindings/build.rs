fn main() {
    windows::build! {
        Windows::Win32::Foundation::{
            CloseHandle,
            HANDLE,
            HINSTANCE,
            HWND,
            PSTR,
            WPARAM,
            LPARAM,
            LRESULT
        },
        Windows::Win32::System::{
            DataExchange::{
                AddClipboardFormatListener,
                GetClipboardOwner
            },
            LibraryLoader::GetModuleHandleA,
            Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS},
            ProcessStatus::K32GetProcessImageFileNameA
        },
        Windows::Win32::UI::WindowsAndMessaging::*
    };
}
