//! Global application settings management.

use lazy_static::lazy_static;
use platform_dirs::{AppDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::RwLock;

/// The directory within `%APPDATA%` to store settings in.
const SETTINGS_DIR: &str = "snip-and-autosave";

/// The name of the file within [`SETTINGS_DIR`] to store global settings in.
///
/// [`SETTINGS_DIR`]: SETTINGS_DIR
const SETTINGS_FILE: &str = "settings.toml";

/// Top-level global settings object.
///
/// Each object stored within this object is de/serialised from a separate TOML
/// section in the settings file.
#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    /// General program configuration.
    pub program: Program,

    /// Paths used by the application.
    pub paths: Paths,
}

/// General program configuration.
#[derive(Serialize, Deserialize)]
pub struct Program {
    /// Whether or not to start the program automatically when the user logs in.
    pub auto_start: bool,
}

impl Default for Program {
    fn default() -> Self {
        Self { auto_start: false }
    }
}

/// Container for paths used by the application.
#[derive(Serialize, Deserialize)]
pub struct Paths {
    /// Where captured screenshots should be saved.
    pub screenshots: PathBuf,
}

impl Default for Paths {
    fn default() -> Self {
        let user_dirs = UserDirs::new().expect("Could not generate UserDirs");

        Self {
            screenshots: user_dirs.picture_dir.join("Screenshots"),
        }
    }
}

lazy_static! {
    /// Global settings object.
    static ref SETTINGS: RwLock<Option<Settings>> = RwLock::new(None);
}

impl Settings {
    /// Reads settings from disk, if this method is being called for the first
    /// time, then calls `f` with the loaded [`Settings`] object, so that
    /// application code can apply various settings, by copying values out.
    ///
    /// [`Settings`]: Settings
    pub fn read(f: impl FnOnce(&Settings)) {
        {
            let reader = SETTINGS.read().unwrap();

            if let Some(ref settings) = *reader {
                f(settings);
                return;
            }
        }

        read_settings();
        Self::read(f);
    }

    /// Writes settings to disk, by calling `f` with a mutable [`Settings`]
    /// reference, then serialising the instance to disk once `f` returns.
    ///
    /// Calls [`read`] before calling `f`, if the application has only just
    /// started, and the settings have not yet been read.
    ///
    /// [`Settings`]: Settings
    /// [`read`]: read
    pub fn write(f: impl FnOnce(&mut Settings)) {
        // Force settings to be read, if this is the first time being called
        Settings::read(|_| {});

        {
            let mut writer = SETTINGS.write().unwrap();

            if let Some(ref mut settings) = *writer {
                f(settings);
            }
        }

        write_settings();
    }
}

/// Returns the fully qualified path to the TOML file that settings should
/// loaded from / stored in.
fn settings_file_path() -> PathBuf {
    let app_dirs = AppDirs::new(Some(SETTINGS_DIR), false).expect("Could not generate AppDirs");

    app_dirs.config_dir.join(SETTINGS_FILE)
}

/// Opens the settings file, then deserialises the TOML configuration within.
///
/// If the settings file does not exist, a [`Default`] instance is created,
/// then written to disk.
///
/// [`Default`]: Default
fn read_settings() {
    let file_path = settings_file_path();

    if file_path.exists() {
        let mut settings_str = String::new();

        File::open(file_path)
            .expect("Unable to open settings.toml")
            .read_to_string(&mut settings_str)
            .expect("Unable to read from settings.toml");

        let mut writer = SETTINGS.write().unwrap();
        *writer = Some(toml::from_str(&settings_str).expect("Failed to parse settings.toml"));
    } else {
        {
            let settings = Settings::default();
            let mut writer = SETTINGS.write().unwrap();

            *writer = Some(settings);
        }

        write_settings();
    }
}

/// Opens the settings file, the serialises the global application settings into
/// it.
fn write_settings() {
    let file_path = settings_file_path();
    let reader = SETTINGS.read().unwrap();

    if !file_path.parent().unwrap().exists() {
        create_dir_all(file_path.parent().unwrap())
            .expect("Unable to create parent directories for settings.toml");
    }

    if let Some(ref settings) = *reader {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
            .expect("Unable to open settings.toml")
            .write_all(
                toml::to_string_pretty(&settings)
                    .expect("Failed to serialise settings")
                    .as_bytes(),
            )
            .expect("Unable to write to settings.toml");
    }
}
