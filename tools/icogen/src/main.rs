// Build-time helper: turns the master logo art into the Windows icon set and the
// raw RGBA bitmap the app loads at runtime.
//
// usage: icogen <source-image> <assets-dir>

use std::path::{Path, PathBuf};

use image::{imageops::FilterType, RgbaImage};

const SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

fn main() {
    if let Err(error) = run() {
        eprintln!("icogen failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = PathBuf::from(args.next().expect("missing source image"));
    let assets = PathBuf::from(args.next().expect("missing assets directory"));
    std::fs::create_dir_all(&assets)?;

    let img = image::open(&source)?.to_rgba8();
    let (width, height) = img.dimensions();
    let side = width.min(height);

    // The generated art carries a light rim around the plate; trim it so the
    // rounded mask lands on the plate itself.
    let inset = (side as f32 * 0.014).round() as u32;
    let cropped = image::imageops::crop_imm(&img, inset, inset, side - inset * 2, side - inset * 2)
        .to_image();

    let mut plate = resize(&cropped, 1024);
    round_corners(&mut plate, 0.235);

    let mut frames: Vec<(u32, Vec<u8>)> = Vec::new();
    for size in SIZES {
        let frame = resize(&plate, size);
        let mut png = Vec::new();
        frame.write_to(
            &mut std::io::Cursor::new(&mut png),
            image::ImageFormat::Png,
        )?;
        frames.push((size, png));
    }

    let ico = build_ico(&frames);
    std::fs::write(assets.join("clutchcore.ico"), &ico)?;

    let large = resize(&plate, 256);
    std::fs::write(assets.join("logo256.rgba"), large.as_raw())?;
    large.save(assets.join("logo256.png"))?;

    println!(
        "icogen: {} frames, ico {} bytes, rgba {} bytes -> {}",
        frames.len(),
        ico.len(),
        large.as_raw().len(),
        assets.display()
    );
    Ok(())
}

fn resize(img: &RgbaImage, size: u32) -> RgbaImage {
    image::imageops::resize(img, size, size, FilterType::Lanczos3)
}

fn round_corners(img: &mut RgbaImage, ratio: f32) {
    let (width, height) = img.dimensions();
    let radius = (ratio * width as f32).min(width as f32 / 2.0).min(height as f32 / 2.0);
    let wf = width as f32;
    let hf = height as f32;
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let cx = fx.clamp(radius, wf - radius);
            let cy = fy.clamp(radius, hf - radius);
            let distance = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let coverage = ((radius - distance) + 0.5).clamp(0.0, 1.0);
            if coverage < 1.0 {
                let pixel = img.get_pixel_mut(x, y);
                pixel[3] = (pixel[3] as f32 * coverage).round() as u8;
            }
        }
    }
}

fn build_ico(frames: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4096 + frames.iter().map(|f| f.1.len()).sum::<usize>());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());

    let mut offset = 6 + 16 * frames.len() as u32;
    let mut directory = Vec::with_capacity(16 * frames.len());
    let mut data = Vec::new();
    for (size, png) in frames {
        let dimension = if *size >= 256 { 0u8 } else { *size as u8 };
        directory.push(dimension);
        directory.push(dimension);
        directory.push(0);
        directory.push(0);
        directory.extend_from_slice(&1u16.to_le_bytes());
        directory.extend_from_slice(&32u16.to_le_bytes());
        directory.extend_from_slice(&(png.len() as u32).to_le_bytes());
        directory.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
        data.extend_from_slice(png);
    }
    out.extend_from_slice(&directory);
    out.extend_from_slice(&data);
    out
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
