use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::{env, fs};
use usvg::{FitTo, SystemFontDB};

#[cfg(not(target_os = "windows"))]
compile_error!("This program only supports Windows platforms");

fn main() {
    // Tell Cargo to only rerun this build script if it, or any other files it depends on change
    println!("cargo:rerun-if-changed=build.rs");

    let icon_path = generate_icon(Path::new("resources/icon.svg"));

    compile_windows_resources(&icon_path);
    compile_windows_manifest();
}

fn generate_icon(svg_path: &Path) -> PathBuf {
    // Tell Cargo to rerun the build script whenever the icon SVG is modified
    println!("cargo:rerun-if-changed={}", svg_path.display());

    // Read / parse SVG icon
    let mut svg_options = usvg::Options::default();

    svg_options.resources_dir = svg_path.parent().map(|p| p.to_path_buf());
    svg_options.fontdb.load_system_fonts();
    svg_options.fontdb.set_generic_families();

    let svg_data = fs::read(svg_path).unwrap();
    let svg = usvg::Tree::from_data(&svg_data, &svg_options).unwrap();

    // Prepare ico renderer
    let mut icon_dir = IconDir::new(ResourceType::Icon);

    // Render icon at all required sizes
    for &size in &[16, 24, 32, 48, 64, 96, 128, 256] {
        let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();

        resvg::render(&svg, FitTo::Size(size, size), pixmap.as_mut()).unwrap();

        let image = IconImage::from_rgba_data(size, size, pixmap.take());
        icon_dir.add_entry(IconDirEntry::encode(&image).unwrap());
    }

    // Write ico to disk
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let icon_path = Path::new(&out_dir).join("icon").with_extension("ico");

    let icon_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&icon_path)
        .unwrap();

    icon_dir.write(icon_file).unwrap();

    icon_path
}

fn compile_windows_resources(icon_path: &Path) {
    let mut res = winres::WindowsResource::new();
    res.set_icon_with_id(icon_path.to_str().unwrap(), "IDI_APPLICATION_ICON");
    res.compile().unwrap();
}

fn compile_windows_manifest() {
    // Tell Cargo to rerun the build script whenever the manifest is modified
    println!("cargo:rerun-if-changed=snip-and-autosave-manifest.rc");
    println!("cargo:rerun-if-changed=snip-and-autosave.exe.manifest");

    embed_resource::compile("snip-and-autosave-manifest.rc");
}
