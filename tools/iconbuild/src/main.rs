//! Сборка иконок приложения из SVG.
//!
//! Отдельный инструмент, в сборку приложения не входит. Берёт `assets/icon.svg`,
//! пишет PNG нужных размеров, `icon.ico` для ярлыка и сырые RGBA для трея.
//!
//! ```text
//! cargo run --manifest-path tools/iconbuild/Cargo.toml -- \
//!   assets/icon.svg assets 16 24 32 48 64 128 180 256 512 \
//!   --ico icon.ico --rgba 16 24 32
//! ```

use std::path::Path;

type Error = Box<dyn std::error::Error>;

fn main() {
    if let Err(err) = run() {
        eprintln!("iconbuild: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let svg_path = args.first().ok_or("нужен путь к SVG")?;
    let out_dir = Path::new(args.get(1).ok_or("нужна папка для результата")?);
    std::fs::create_dir_all(out_dir)?;

    let mut sizes = Vec::new();
    let mut rgba_sizes = Vec::new();
    let mut ico_name: Option<String> = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--ico" => {
                ico_name = Some(args.get(index + 1).ok_or("--ico без имени файла")?.clone());
                index += 2;
            }
            "--rgba" => {
                index += 1;
                while index < args.len() && !args[index].starts_with("--") {
                    rgba_sizes.push(args[index].parse::<u32>()?);
                    index += 1;
                }
            }
            value => {
                sizes.push(value.parse::<u32>()?);
                index += 1;
            }
        }
    }

    let source = std::fs::read_to_string(svg_path)?;
    let tree = resvg::usvg::Tree::from_str(&source, &resvg::usvg::Options::default())?;

    let mut rendered = Vec::new();
    for size in sizes {
        let pixmap = render(&tree, size)?;
        pixmap.save_png(out_dir.join(format!("icon-{size}.png")))?;
        rendered.push((size, pixmap));
    }

    for size in rgba_sizes {
        let pixmap = match rendered.iter().find(|(rendered, _)| *rendered == size) {
            Some((_, pixmap)) => pixmap.clone(),
            None => render(&tree, size)?,
        };
        let name = format!("tray-{size}.rgba");
        std::fs::write(out_dir.join(&name), pixmap.data())?;
        println!("{name}");
    }

    if let Some(name) = ico_name {
        let mut images = Vec::new();
        for (size, pixmap) in &rendered {
            images.push((*size, ico_entry(*size, pixmap)?));
        }
        std::fs::write(out_dir.join(&name), build_ico(&images))?;
        println!("{name}");
    }
    Ok(())
}

fn render(tree: &resvg::usvg::Tree, size: u32) -> Result<resvg::tiny_skia::Pixmap, Error> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or("не удалось создать холст")?;
    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

/// Запись значка для `.ico`: маленькие размеры — DIB, крупные — PNG.
fn ico_entry(size: u32, pixmap: &resvg::tiny_skia::Pixmap) -> Result<Vec<u8>, Error> {
    if size > 64 {
        let mut buffer = Vec::new();
        let mut encoder = png::Encoder::new(&mut buffer, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        {
            let mut writer = encoder.write_header()?;
            writer.write_image_data(pixmap.data())?;
        }
        Ok(buffer)
    } else {
        Ok(dib(pixmap))
    }
}

fn dib(pixmap: &resvg::tiny_skia::Pixmap) -> Vec<u8> {
    let width = pixmap.width();
    let height = pixmap.height();
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&((height as i32) * 2).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(width * height * 4).to_le_bytes());
    for _ in 0..4 {
        out.extend_from_slice(&0i32.to_le_bytes());
    }
    let data = pixmap.data();
    for y in (0..height as usize).rev() {
        for x in 0..width as usize {
            let pixel = (y * width as usize + x) * 4;
            out.extend_from_slice(&[
                data[pixel + 2],
                data[pixel + 1],
                data[pixel],
                data[pixel + 3],
            ]);
        }
    }
    let mask_row = (width as usize).div_ceil(32) * 4;
    out.resize(out.len() + mask_row * height as usize, 0);
    out
}

fn build_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in images {
        let dimension = if *size >= 256 { 0 } else { *size as u8 };
        out.push(dimension);
        out.push(dimension);
        out.push(0);
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend_from_slice(data);
    }
    out
}
