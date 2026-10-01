use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut rc = format!(
        "1 ICON \"{}\"\n",
        root.join("assets/app.ico")
            .display()
            .to_string()
            .replace('\\', "\\\\")
    );
    for (id, brand, color) in [
        (2, "intel", "#8AD9FF"),
        (3, "amd", "#FFFFFF"),
        (4, "nvidia", "#76B900"),
    ] {
        let svg = fs::read_to_string(root.join(format!("assets/brands/{brand}.svg")))
            .unwrap()
            .replace("<svg ", &format!("<svg fill=\"{color}\" "));
        let tree =
            resvg::usvg::Tree::from_data(svg.as_bytes(), &resvg::usvg::Options::default()).unwrap();
        // Supersample each DPI size at build time; runtime still draws at its exact size.
        let sizes = [28u32, 35, 42, 49, 56, 63, 70, 84, 112];
        let images: Vec<_> = sizes
            .iter()
            .map(|&size| {
                const SAMPLES: u32 = 4;
                let large_size = size * SAMPLES;
                let mut large = resvg::tiny_skia::Pixmap::new(large_size, large_size).unwrap();
                // Composite onto an opaque navy plate before filtering. This avoids
                // transparent edge pixels depending on the window background.
                large.fill(resvg::tiny_skia::Color::from_rgba8(0x10, 0x1B, 0x2C, 255));
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::from_scale(
                        large_size as f32 / tree.size().width(),
                        large_size as f32 / tree.size().height(),
                    ),
                    &mut large.as_mut(),
                );
                let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).unwrap();
                // Exact area average of each 4x4 block. All channels are premultiplied
                // and the background is opaque, so no alpha unpremultiplication is needed.
                for y in 0..size {
                    for x in 0..size {
                        let mut sum = [0u32; 4];
                        for sy in 0..SAMPLES {
                            for sx in 0..SAMPLES {
                                let offset = (((y * SAMPLES + sy) * large_size + x * SAMPLES + sx)
                                    * 4) as usize;
                                for (channel, total) in sum.iter_mut().enumerate() {
                                    *total += large.data()[offset + channel] as u32;
                                }
                            }
                        }
                        let offset = ((y * size + x) * 4) as usize;
                        for (channel, total) in sum.into_iter().enumerate() {
                            pixmap.data_mut()[offset + channel] =
                                ((total + SAMPLES * SAMPLES / 2) / (SAMPLES * SAMPLES)) as u8;
                        }
                    }
                }
                pixmap.encode_png().unwrap()
            })
            .collect();
        let mut ico = Vec::new();
        for value in [0u16, 1, sizes.len() as u16] {
            ico.extend(value.to_le_bytes());
        }
        let mut offset = 6 + 16 * sizes.len() as u32;
        for (&size, png) in sizes.iter().zip(&images) {
            ico.extend([size as u8, size as u8, 0, 0]);
            ico.extend(1u16.to_le_bytes());
            ico.extend(32u16.to_le_bytes());
            ico.extend((png.len() as u32).to_le_bytes());
            ico.extend(offset.to_le_bytes());
            offset += png.len() as u32;
        }
        for png in images {
            ico.extend(png);
        }
        let path = out.join(format!("{brand}.ico"));
        fs::write(&path, ico).unwrap();
        rc.push_str(&format!(
            "{id} ICON \"{}\"\n",
            path.display().to_string().replace('\\', "\\\\")
        ));
    }
    let path = out.join("app.rc");
    fs::write(&path, rc).unwrap();
    embed_resource::compile(path, embed_resource::NONE)
        .manifest_required()
        .expect("Windows resources");
}
