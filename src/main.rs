use crate::convert::dib_to_image;
use crate::heuristics::clipboard_owned_by_snip_and_sketch;
use crate::windows::{
    add_clipboard_listener, create_window, create_window_class, get_clipboard_data, get_instance,
    message_loop, open_clipboard,
};
use bindings::Windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::BITMAPINFO,
    System::SystemServices::CF_DIB,
    UI::WindowsAndMessaging::{DefWindowProcA, WM_CLIPBOARDUPDATE},
};
use image::ImageFormat;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use win32_notification::NotificationBuilder;

mod convert;
mod heuristics;
mod windows;

fn debounce_message(message: u32) -> bool {
    const DEBOUNCE_TIME: Duration = Duration::from_millis(500);

    lazy_static! {
        static ref MESSAGE_TIMES: Mutex<HashMap<u32, Instant>> = Mutex::new(HashMap::new());
    }

    let mut message_times = (*MESSAGE_TIMES).lock().unwrap();

    let result = if let Some(message_time) = message_times.get(&message) {
        Instant::now().duration_since(*message_time) <= DEBOUNCE_TIME
    } else {
        false
    };

    message_times.insert(message, Instant::now());

    result
}

fn show_screenshot_saved_notification(info_text: Option<&str>) {
    let notification = NotificationBuilder::new()
        .title_text("Screenshot saved")
        .info_text(info_text.unwrap_or("lmao xd"))
        .build()
        .expect("Could not create notification");

    notification.show().expect("Failed to show notification");
}

// noinspection RsUnreachablePatterns
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match message {
        WM_CLIPBOARDUPDATE => {
            println!("WM_CLIPBOARDUPDATE message received");

            if debounce_message(WM_CLIPBOARDUPDATE) {
                println!("WM_CLIPBOARDUPDATE debounced - message ignored");
                return LRESULT(0);
            }

            if clipboard_owned_by_snip_and_sketch().unwrap_or_else(|e| {
                println!("Heuristics failed: {:#?}", e);
                false
            }) {
                println!("Clipboard is owned by Snip & Sketch - saving screenshot to disk");

                // Give the Snip & Sketch screenshot overlay a chance to
                // disappear before we block the clipboard to copy image data
                thread::sleep(Duration::from_millis(100));

                let image = {
                    let _clipboard = open_clipboard(Some(window)).unwrap();
                    let bitmap = get_clipboard_data::<BITMAPINFO>(CF_DIB).unwrap();

                    dib_to_image(bitmap).unwrap()
                };

                thread::spawn(move || {
                    image
                        .save_with_format(Path::new("./lmao.png"), ImageFormat::Png)
                        .unwrap()
                });
            } else {
                println!("Clipboard not owned by Snip & Sketch");
            }

            LRESULT(0)
        }
        _ => DefWindowProcA(window, message, w_param, l_param),
    }
}

fn main() -> ::windows::Result<()> {
    // Create a hidden window, so we can receive clipboard messages
    let instance = get_instance()?;
    let class = create_window_class(instance, Some(window_proc))?;
    let window = create_window(instance, &class)?;

    // Register our hidden window as a clipboard listener
    add_clipboard_listener(window)?;

    // Await clipboard messages indefinitely
    message_loop(window);

    Ok(())
}
