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
        // Render each common DPI size directly from SVG, avoiding resampling at runtime.
        let sizes = [28u32, 35, 42, 49, 56, 63, 70, 84, 112];
        let images: Vec<_> = sizes
            .iter()
            .map(|&size| {
                let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::from_scale(
                        size as f32 / tree.size().width(),
                        size as f32 / tree.size().height(),
                    ),
                    &mut pixmap.as_mut(),
                );
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
