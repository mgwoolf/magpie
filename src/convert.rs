//! SVG rasterising and image decoding.

use std::sync::OnceLock;

use resvg::{tiny_skia, usvg};

/// RGBA8 pixels, premultiplied alpha.
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba_premul: Vec<u8>,
}

impl Raster {
    pub fn has_transparency(&self) -> bool {
        self.rgba_premul.chunks_exact(4).any(|p| p[3] < 255)
    }
}

fn options() -> &'static usvg::Options<'static> {
    static OPTS: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPTS.get_or_init(|| {
        let mut opt = usvg::Options::default();
        // Needed for logos that contain <text> elements.
        opt.fontdb_mut().load_system_fonts();
        opt
    })
}

fn parse(svg: &[u8]) -> Result<usvg::Tree, String> {
    usvg::Tree::from_data(svg, options()).map_err(|e| format!("invalid SVG: {e}"))
}

fn render(tree: &usvg::Tree, width: u32, height: u32) -> Result<tiny_skia::Pixmap, String> {
    let size = tree.size();
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("output size must be non-zero")?;
    let transform = tiny_skia::Transform::from_scale(width as f32 / size.width(), height as f32 / size.height());
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Output dimensions for an SVG rendered to `width` pixels wide, keeping aspect ratio.
fn scaled(tree: &usvg::Tree, width: u32) -> (u32, u32) {
    let size = tree.size();
    let height = (width as f32 * size.height() / size.width()).round().max(1.0) as u32;
    (width.max(1), height)
}

/// Convert an SVG to a transparent PNG `width` pixels wide (height follows aspect ratio).
pub fn svg_to_png(svg: &[u8], width: u32) -> Result<Vec<u8>, String> {
    let tree = parse(svg)?;
    let (w, h) = scaled(&tree, width);
    render(&tree, w, h)?
        .encode_png()
        .map_err(|e| format!("PNG encode failed: {e}"))
}

/// Full-size straight-alpha RGBA pixels: SVGs are rendered `svg_width` wide,
/// PNGs are decoded at their native size. Used for clipboard copies.
pub fn rgba(data: &[u8], is_svg: bool, svg_width: u32) -> Result<(u32, u32, Vec<u8>), String> {
    if is_svg {
        let tree = parse(data)?;
        let (w, h) = scaled(&tree, svg_width);
        let pixmap = render(&tree, w, h)?;
        let bytes = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        return Ok((w, h, bytes));
    }
    let img = image::load_from_memory(data)
        .map_err(|e| format!("invalid image: {e}"))?
        .to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

/// Rasterise SVG or decode PNG so that it fits in a `max`×`max` box, for previews.
pub fn preview(data: &[u8], is_svg: bool, max: u32) -> Result<Raster, String> {
    if is_svg {
        let tree = parse(data)?;
        let size = tree.size();
        let scale = max as f32 / size.width().max(size.height());
        let w = (size.width() * scale).round().max(1.0) as u32;
        let h = (size.height() * scale).round().max(1.0) as u32;
        let pixmap = render(&tree, w, h)?;
        return Ok(Raster {
            width: w,
            height: h,
            rgba_premul: pixmap.take(),
        });
    }

    let img = image::load_from_memory(data).map_err(|e| format!("invalid image: {e}"))?;
    let img = if img.width() > max || img.height() > max {
        img.thumbnail(max, max)
    } else {
        img
    };
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let a = p[3] as u16;
        for c in &mut p.0[..3] {
            *c = ((*c as u16 * a + 127) / 255) as u8;
        }
    }
    Ok(Raster {
        width: rgba.width(),
        height: rgba.height(),
        rgba_premul: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><circle cx="5" cy="5" r="5" fill="red"/></svg>"#;

    #[test]
    fn converts_with_aspect_and_transparency() {
        let png = svg_to_png(SVG, 200).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!((img.width(), img.height()), (200, 100));
        assert_eq!(img.get_pixel(199, 0)[3], 0, "background should be transparent");
        assert_eq!(img.get_pixel(50, 50).0, [255, 0, 0, 255]);
    }

    #[test]
    fn rgba_is_straight_alpha() {
        let (w, h, px) = rgba(SVG, true, 40).unwrap();
        assert_eq!((w, h, px.len()), (40, 20, 40 * 20 * 4));
        assert_eq!(&px[(10 * 40 + 10) * 4..][..4], &[255, 0, 0, 255]);
        assert_eq!(px[(39) * 4 + 3], 0);
    }

    #[test]
    fn preview_fits_box() {
        let r = preview(SVG, true, 64).unwrap();
        assert_eq!((r.width, r.height), (64, 32));
        assert!(r.has_transparency());
    }
}
