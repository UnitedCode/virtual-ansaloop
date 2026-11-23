use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::Command,
};
mod layout;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn run(command: &mut Command) -> Result<()> {
    if !command.status()?.success() {
        return Err(format!("command failed: {command:?}").into());
    }
    Ok(())
}
fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    std::env::set_current_dir(root)?;
    match std::env::args().nth(1).as_deref() {
        Some("build") => {
            layout::generate(root)?;
            run(Command::new("cargo").args([
                "build",
                "--release",
                "--locked",
                "-p",
                "virtual-ansaloop-web",
                "--target",
                "wasm32-unknown-unknown",
            ]))?;
            let target = std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join("target"));
            fs::create_dir_all("dist")?;
            run(Command::new("wasm-bindgen")
                .arg(target.join("wasm32-unknown-unknown/release/virtual_ansaloop_web.wasm"))
                .args([
                    "--target",
                    "web",
                    "--out-dir",
                    "dist",
                    "--out-name",
                    "ansaloop",
                    "--no-typescript",
                ]))?;
            for file in [
                "index.html",
                ".nojekyll",
                "style.css",
                "worker.js",
                "audio-worklet.js",
                "easa-phone-kx-t1421.jpg",
                "easa-phone-straightened.png",
                "endless-cassette.png",
                "sequencer-module.svg",
                "sequencer-pcb.svg",
                "sequencer-layout.svg",
            ] {
                fs::copy(root.join("assets").join(file), root.join("dist").join(file))?;
            }
            fs::write(
                "dist/firmware-sha256.txt",
                virtual_hash(&fs::read("firmware/ansaloop.elf")?),
            )?;
            println!(
                "Built separate browser instrument in {}",
                root.join("dist").display()
            );
        }
        Some("serve") => {
            let listener = TcpListener::bind("127.0.0.1:3030")?;
            let root = root.join("dist").canonicalize()?;
            println!("Virtual AnsaLoop: http://127.0.0.1:3030");
            for stream in listener.incoming() {
                let mut stream = stream?;
                stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
                let mut bytes = [0; 8192];
                let Ok(n) = stream.read(&mut bytes) else {
                    continue;
                };
                let url = std::str::from_utf8(&bytes[..n])
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .split('?')
                    .next()
                    .unwrap_or("/");
                let path = root
                    .join(if url == "/" {
                        "index.html"
                    } else {
                        url.trim_start_matches('/')
                    })
                    .canonicalize();
                let (status, body, mime) = match path {
                    Ok(p) if p.starts_with(&root) && p.is_file() => {
                        let mime = match p.extension().and_then(|e| e.to_str()) {
                            Some("html") => "text/html; charset=utf-8",
                            Some("js") => "text/javascript",
                            Some("css") => "text/css",
                            Some("wasm") => "application/wasm",
                            Some("jpg") | Some("jpeg") => "image/jpeg",
                            Some("png") => "image/png",
                            Some("svg") => "image/svg+xml",
                            _ => "application/octet-stream",
                        };
                        ("200 OK", fs::read(p)?, mime)
                    }
                    _ => ("404 Not Found", b"Not found".to_vec(), "text/plain"),
                };
                let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len());
                let _ = stream.write_all(&body);
            }
        }
        Some("fixture") => {
            fs::create_dir_all("results")?;
            let frequency = std::env::args()
                .nth(2)
                .map(|v| v.parse::<f64>())
                .transpose()?
                .unwrap_or(220.);
            if !frequency.is_finite() || !(40. ..=4000.).contains(&frequency) {
                return Err("fixture frequency must be 40–4000 Hz".into());
            }
            let path = format!("results/input-{frequency:.0}.wav");
            let mut writer = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 48000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )?;
            for i in 0..297600 {
                let t = f64::from(i) / 48000.;
                let sample = 0.3 * (std::f64::consts::TAU * (frequency * t + 4.731 * t * t)).sin();
                writer.write_sample((sample * 32767.) as i16)?;
            }
            writer.finalize()?;
            println!("{}", root.join(path).display());
        }
        Some("layout") => {
            layout::generate(root)?;
        }
        Some("firmware") => {
            run(Command::new("rustc")
                .args([
                    "--edition=2021",
                    "--target",
                    "thumbv7em-none-eabihf",
                    "-C",
                    "target-cpu=cortex-m7",
                    "-C",
                    "opt-level=s",
                    "-C",
                    "panic=abort",
                    "-C",
                ])
                .arg(format!(
                    "link-arg=-T{}",
                    root.join("firmware/tape.ld").display()
                ))
                .arg(root.join("firmware/dual-tape.rs"))
                .args(["-o", "firmware/ansaloop.elf"]))?;
            println!(
                "ARM ELF SHA256: {}",
                virtual_hash(&fs::read("firmware/ansaloop.elf")?)
            );
        }
        _ => return Err("use: cargo run --release -p xtask -- build | serve | firmware".into()),
    }
    Ok(())
}
fn virtual_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
