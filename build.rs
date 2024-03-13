use std::{
    borrow::Cow,
    env,
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

fn try_link_pdfium() -> Result<(), Box<dyn Error>> {
    let bin_ext = [
        OsStr::new("dll"),
        OsStr::new("lib"),
        OsStr::new("so"),
        OsStr::new("a"),
    ];
    let mut target_path = PathBuf::from(env::var("OUT_DIR")?);
    target_path.push("pdfium_binaries");
    fs::create_dir_all(target_path.as_path())?;
    let has_bin = fs::read_dir(target_path.as_path())?.any(|x| {
        bin_ext.iter().any(|s| {
            x.as_ref()
                .ok()
                .and_then(|d| {
                    if Path::new(d.file_name().as_os_str()).extension() == Some(s) {
                        Some(())
                    } else {
                        None
                    }
                })
                .is_some()
        })
    });
    if !has_bin {
        let build_id = 4660;
        #[cfg(all(windows, target_arch = "x86"))]
        let platform_name = "windows-x86";
        #[cfg(all(windows, target_arch = "x86_64"))]
        let platform_name = "windows-x64";
        let binary_package_url = format!("https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F{}/pdfium-{}.tgz", build_id, platform_name);
        let resp = reqwest::blocking::get(binary_package_url)?;
        let tar = GzDecoder::new(resp);
        let mut archive = Archive::new(tar);
        for entry in archive.entries()? {
            let mut file = entry?;
            if file.header().entry_type() == EntryType::Regular {
                let file_path = file.path()?;
                let file_extension = file_path.extension();
                let is_bin = bin_ext.iter().any(|s| file_extension == Some(s));
                if is_bin {
                    if let Some(file_name_raw) = file_path.file_name() {
                        let mut file_name_str = file_name_raw.to_string_lossy();
                        if let Some(s) = file_name_str.strip_suffix(".dll.lib") {
                            file_name_str = Cow::Owned(s.to_owned() + ".lib");
                        }
                        target_path.push(file_name_str.as_ref());
                        file.unpack(target_path.as_path())?;
                        target_path.pop();
                    }
                }
            }
        }
    }

    println!(
        "cargo:rustc-link-search=native={}",
        target_path.to_str().unwrap()
    );
    println!("cargo:rustc-link-lib=dylib=pdfium");
    Ok(())
}

fn main() {
    #[cfg(windows)]
    {
        #[cfg(feature = "pdfium")]
        try_link_pdfium().unwrap();
    }
}
