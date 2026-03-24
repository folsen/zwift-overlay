use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN_NAME: &str = "zwift_overlay";
const ICON_PNGS: &[&str] = &[
    "assets/app.iconset/icon_16x16.png",
    "assets/app.iconset/icon_32x32.png",
    "assets/app.iconset/icon_32x32@2x.png",
    "assets/app.iconset/icon_128x128.png",
    "assets/app.iconset/icon_256x256.png",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for path in ICON_PNGS {
        println!("cargo:rerun-if-changed={path}");
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    match embed_windows_icon() {
        Ok(()) => {}
        Err(e) => {
            println!("cargo:warning=Skipping Windows icon embedding: {e}");
        }
    }
}

fn embed_windows_icon() -> io::Result<()> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    let icon_path = out_dir.join("app-icon.ico");
    let rc_path = out_dir.join("app-icon.rc");

    write_ico(&icon_path)?;
    fs::write(&rc_path, "1 ICON \"app-icon.ico\"\n")?;

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let resource_path = match target_env.as_str() {
        "msvc" => compile_with_rc(&out_dir, &rc_path)?,
        "gnu" => compile_with_windres(&out_dir, &rc_path)?,
        other => {
            return Err(io::Error::other(format!(
                "unsupported Windows target environment: {other}"
            )));
        }
    };

    println!(
        "cargo:rustc-link-arg-bin={BIN_NAME}={}",
        resource_path.display()
    );
    Ok(())
}

fn write_ico(destination: &Path) -> io::Result<()> {
    struct IconEntry {
        width: u8,
        height: u8,
        png: Vec<u8>,
    }

    let entries = ICON_PNGS
        .iter()
        .map(|path| {
            let png = fs::read(path)?;
            let (width, height) = icon_dimensions(path)?;
            Ok(IconEntry { width, height, png })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let mut file = fs::File::create(destination)?;
    write_u16(&mut file, 0)?;
    write_u16(&mut file, 1)?;
    write_u16(&mut file, entries.len() as u16)?;

    let mut offset = 6 + (entries.len() as u32 * 16);
    for entry in &entries {
        file.write_all(&[entry.width, entry.height, 0, 0])?;
        write_u16(&mut file, 1)?;
        write_u16(&mut file, 32)?;
        write_u32(&mut file, entry.png.len() as u32)?;
        write_u32(&mut file, offset)?;
        offset += entry.png.len() as u32;
    }

    for entry in &entries {
        file.write_all(&entry.png)?;
    }

    Ok(())
}

fn icon_dimensions(path: &str) -> io::Result<(u8, u8)> {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other(format!("invalid icon path: {path}")))?;

    let logical_size = if file_name.contains("16x16") {
        16
    } else if file_name.contains("32x32@2x") {
        64
    } else if file_name.contains("32x32") {
        32
    } else if file_name.contains("128x128") {
        128
    } else if file_name.contains("256x256") {
        256
    } else {
        return Err(io::Error::other(format!(
            "unsupported icon filename: {file_name}"
        )));
    };

    let byte = if logical_size == 256 {
        0
    } else {
        logical_size as u8
    };
    Ok((byte, byte))
}

fn compile_with_rc(out_dir: &Path, rc_path: &Path) -> io::Result<PathBuf> {
    let resource_path = out_dir.join("app-icon.res");
    run(Command::new("rc.exe")
        .current_dir(out_dir)
        .arg("/nologo")
        .arg(format!("/fo{}", resource_path.display()))
        .arg(
            rc_path
                .file_name()
                .expect("resource script path should have a file name"),
        ))?;
    Ok(resource_path)
}

fn compile_with_windres(out_dir: &Path, rc_path: &Path) -> io::Result<PathBuf> {
    let object_path = out_dir.join("app-icon.o");
    run(Command::new("windres")
        .current_dir(out_dir)
        .arg("--input-format=rc")
        .arg("--output-format=coff")
        .arg("--output")
        .arg(&object_path)
        .arg(
            rc_path
                .file_name()
                .expect("resource script path should have a file name"),
        ))?;
    Ok(object_path)
}

fn run(command: &mut Command) -> io::Result<()> {
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }

    Err(io::Error::other(format!(
        "command failed: {}\nstdout:\n{}\nstderr:\n{}",
        format_command(command),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn format_command(command: &Command) -> String {
    let mut rendered = command.get_program().to_string_lossy().to_string();
    for arg in command.get_args() {
        rendered.push(' ');
        rendered.push_str(&arg.to_string_lossy());
    }
    rendered
}

fn write_u16(writer: &mut fs::File, value: u16) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u32(writer: &mut fs::File, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}
