use lazy_static::lazy_static;
use platform_dirs::{AppDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    pub paths: Paths,
}

#[derive(Serialize, Deserialize)]
pub struct Paths {
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
    static ref SETTINGS: RwLock<Option<Settings>> = RwLock::new(None);
}

impl Settings {
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

fn settings_file_path() -> PathBuf {
    let app_dirs =
        AppDirs::new(Some("snip-and-autosave"), false).expect("Could not generate AppDirs");

    app_dirs.config_dir.join("settings.toml")
}

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
