use crate::windows::{get_process_image_file_name, get_window_thread_and_process_id, open_process};
use bindings::Windows::Win32::System::DataExchange::GetClipboardOwner;

fn get_clipboard_owner_process_name() -> windows::Result<String> {
    // TODO maybe move this to `windows.rs`
    let owner_window = unsafe { GetClipboardOwner() };
    let (process, thread) = get_window_thread_and_process_id(owner_window);

    println!(
        "Clipboard contents owned by process {}, thread {}",
        process, thread
    );

    let process_handle = open_process(process)?;
    let process_name = get_process_image_file_name(process_handle.value())?;

    println!("Process name: {}", process_name);

    Ok(process_name)
}

pub fn clipboard_owned_by_snip_and_sketch() -> windows::Result<bool> {
    let process_name = get_clipboard_owner_process_name()?;
    let process_name_heuristic = process_name.ends_with("\\svchost.exe");

    Ok(process_name_heuristic)
}
