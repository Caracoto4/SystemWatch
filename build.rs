use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=icon.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let target_arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() { Ok("x86_64") => "x64", Ok("x86") => "x86", Ok("aarch64") => "arm64", _ => "x64" };
    let sdk_root = PathBuf::from(env::var_os("ProgramFiles(x86)").unwrap_or_else(|| r"C:\Program Files (x86)".into()))
        .join(r"Windows Kits\10\bin");
    let resource_compiler = fs::read_dir(&sdk_root).ok()
        .into_iter().flatten().filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| entry.path().join(&target_arch).join("rc.exe"))
        .find(|path| path.is_file());
    let Some(resource_compiler) = resource_compiler else {
        println!("cargo:warning=Windows SDK rc.exe was not found; the executable will use the default icon.");
        return;
    };

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory")).join("systemwatch.res");
    let status = Command::new(resource_compiler)
        .arg("/nologo")
        .arg("/fo")
        .arg(&output)
        .arg(manifest.join("app.rc"))
        .current_dir(&manifest)
        .status()
        .expect("failed to launch Windows resource compiler");
    if !status.success() {
        panic!("Windows resource compiler failed with {status}");
    }
    println!("cargo:rustc-link-arg={}", output.display());
}