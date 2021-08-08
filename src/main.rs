#![windows_subsystem = "windows"]

use crate::convert::dib_to_image;
use crate::extensions::ImageExtensions;
use crate::heuristics::clipboard_owned_by_snip_and_sketch;
use crate::notification_area::WMAPP_NOTIFYCALLBACK;
use crate::settings::Settings;
use crate::windows::{
    add_clipboard_listener, attach_console, com_initialize, create_window, create_window_class,
    destroy_window, find_window, get_clipboard_dib, get_instance, message_loop, open_clipboard,
    post_quit_message, CLASS_NAME, WINDOW_NAME,
};
use bindings::Windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::Com::COINIT_APARTMENTTHREADED,
    UI::WindowsAndMessaging::{
        DefWindowProcA, WM_CLIPBOARDUPDATE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY,
    },
};
use chrono::Local;
use image::ImageFormat;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{fs, thread};

mod convert;
mod extensions;
mod heuristics;
mod notification_area;
mod settings;
mod windows;

/// Debounces incoming window messages, returning `true` if the debounce period
/// for a specific `message` has been exceeded.
fn debounce_message(message: u32) -> bool {
    const DEBOUNCE_TIME: Duration = Duration::from_millis(1000);

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

/// Generates the fully qualified path for a new screenshot.
fn generate_output_path() -> PathBuf {
    let mut screenshot_path = PathBuf::new();
    Settings::read(|s| screenshot_path = s.paths.screenshots.clone());

    // Make sure that the screenshot path exists, if we are running for the first time
    fs::create_dir_all(&screenshot_path).unwrap();

    let now = Local::now();

    screenshot_path
        .join(format!(
            "Screenshot_{}",
            now.format("%Y%m%d_%H%M%S").to_string()
        ))
        .with_extension("png")
}

/// `WM_CREATE` message processor.
fn on_create(window: HWND) -> LRESULT {
    notification_area::create_icon(window).unwrap();

    LRESULT(0)
}

/// `WM_CLOSE` message processor.
fn on_close(window: HWND) -> LRESULT {
    notification_area::remove_icon(window).unwrap();
    destroy_window(window);

    LRESULT(0)
}

/// `WM_DESTROY` message processor.
fn on_destroy() -> LRESULT {
    post_quit_message(0);

    LRESULT(0)
}

/// `WM_COMMAND` message processor.
///
/// This function defers to different command processors within the program.
fn on_command(window: HWND, message: u32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    let command = w_param.0 & 0xFFFF;

    for command_proc in &[notification_area::on_command] {
        if let Some(result) = command_proc(window, command) {
            return result;
        }
    }

    unsafe { DefWindowProcA(window, message, w_param, l_param) }
}

/// `WM_CLIPBOARDUPDATE` message processor.
fn on_clipboard_update(window: HWND) -> LRESULT {
    println!("\nWM_CLIPBOARDUPDATE message received");

    if debounce_message(WM_CLIPBOARDUPDATE) {
        println!("WM_CLIPBOARDUPDATE debounced - message ignored");
        return LRESULT(0);
    } else if clipboard_owned_by_snip_and_sketch().unwrap_or_else(|e| {
        println!("Heuristics failed: {:#?}", e);
        false
    }) {
        println!("Clipboard is owned by Snip & Sketch - saving screenshot to disk");

        // Give the Snip & Sketch screenshot overlay a chance to
        // disappear before we block the clipboard to copy image data
        thread::sleep(Duration::from_millis(100));

        // TODO: don't unwrap here
        let image = {
            let _clipboard = open_clipboard(Some(window)).unwrap();
            let bitmap = get_clipboard_dib().unwrap();

            dib_to_image(bitmap).unwrap()
        };

        thread::spawn(move || {
            if image.is_same_as_last_screenshot() {
                println!("Screenshot is the same as the last saved image - ignoring");
                return;
            }

            image
                .save_with_format(generate_output_path(), ImageFormat::Png)
                .unwrap();
        });
    } else {
        println!("Clipboard not owned by Snip & Sketch");
    }

    LRESULT(0)
}

/// `wndProc`, i.e. the window message processor.
// noinspection RsLiveness
// noinspection RsUnreachablePatterns
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match message {
        WM_CREATE => on_create(window),
        WM_COMMAND => on_command(window, message, w_param, l_param),
        WM_CLIPBOARDUPDATE => on_clipboard_update(window),
        WMAPP_NOTIFYCALLBACK => notification_area::notify_callback(window, w_param, l_param),
        WM_CLOSE => on_close(window),
        WM_DESTROY => on_destroy(),
        _ => DefWindowProcA(window, message, w_param, l_param),
    }
}

fn main() -> ::windows::Result<()> {
    attach_console();
    com_initialize(COINIT_APARTMENTTHREADED)?;

    // Only allow one instance of the program to run at a time
    if find_window(CLASS_NAME, WINDOW_NAME).is_some() {
        println!("Only one instance of this program can run at a time");
        return Ok(());
    }

    // Create a hidden window, so we can receive clipboard messages
    let instance = get_instance()?;
    let class = create_window_class(instance, CLASS_NAME, Some(window_proc))?;
    let window = create_window(instance, &class, WINDOW_NAME)?;

    // Register our hidden window as a clipboard listener
    add_clipboard_listener(window)?;

    // Await clipboard messages indefinitely
    message_loop(HWND(0));

    Ok(())
}
