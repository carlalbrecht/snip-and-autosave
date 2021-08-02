fn main() {
    windows::build! {
        Windows::Win32::{
            Foundation::{
                CloseHandle,
                HANDLE,
                HINSTANCE,
                HWND,
                PSTR,
                WPARAM,
                LPARAM,
                LRESULT
            },
            System::{
                DataExchange::{
                    AddClipboardFormatListener,
                    GetClipboardData,
                    GetClipboardOwner,
                    GetPriorityClipboardFormat,
                    OpenClipboard,
                    CloseClipboard
                },
                LibraryLoader::GetModuleHandleA,
                Threading::{
                    OpenProcess,
                    PROCESS_ACCESS_RIGHTS
                },
                ProcessStatus::K32GetProcessImageFileNameA,
                SystemServices::CLIPBOARD_FORMATS
            },
            Graphics::Gdi::{BITMAPINFO, BITMAPINFOHEADER, BI_BITFIELDS},
            UI::WindowsAndMessaging::*
        }
    };
}
