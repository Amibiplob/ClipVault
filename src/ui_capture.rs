//! Test-only software rasterization of egui's actual meshes and font atlas.
//! These are layout artifacts, not claims of a real compositor capture.
use eframe::egui::*;
use std::collections::HashMap;
pub fn capture(
    ctx: &Context,
    output: FullOutput,
    textures: &mut HashMap<TextureId, ColorImage>,
    size: [u32; 2],
    path: &std::path::Path,
) {
    for (id, delta) in &output.textures_delta.set {
        let image = match &delta.image {
            ImageData::Color(i) => (**i).clone(),
            ImageData::Font(i) => ColorImage {
                size: i.size,
                pixels: i.srgba_pixels(None).collect(),
            },
        };
        if let Some([x, y]) = delta.pos {
            let target = textures.get_mut(id).expect("existing atlas");
            for row in 0..image.size[1] {
                let from = row * image.size[0];
                let to = (y + row) * target.size[0] + x;
                target.pixels[to..to + image.size[0]]
                    .copy_from_slice(&image.pixels[from..from + image.size[0]]);
            }
        } else {
            textures.insert(*id, image);
        }
    }
    let mut canvas = image::RgbaImage::new(size[0], size[1]);
    for p in canvas.pixels_mut() {
        *p = image::Rgba([0, 0, 0, 255]);
    }
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    for primitive in ctx.tessellate(output.shapes, 1.) {
        let epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let Some(texture) = textures.get(&mesh.texture_id) else {
            continue;
        };
        for indices in mesh.indices.chunks(3) {
            let v = [
                mesh.vertices[indices[0] as usize],
                mesh.vertices[indices[1] as usize],
                mesh.vertices[indices[2] as usize],
            ];
            let area = cross(v[1].pos - v[0].pos, v[2].pos - v[0].pos);
            if area.abs() < 0.0001 {
                continue;
            }
            let bounds = Rect::from_min_max(
                pos2(
                    v.iter().map(|v| v.pos.x).fold(f32::INFINITY, f32::min),
                    v.iter().map(|v| v.pos.y).fold(f32::INFINITY, f32::min),
                ),
                pos2(
                    v.iter().map(|v| v.pos.x).fold(0., f32::max),
                    v.iter().map(|v| v.pos.y).fold(0., f32::max),
                ),
            )
            .intersect(primitive.clip_rect);
            for y in bounds.min.y.max(0.) as u32..bounds.max.y.ceil().min(size[1] as f32) as u32 {
                for x in bounds.min.x.max(0.) as u32..bounds.max.x.ceil().min(size[0] as f32) as u32
                {
                    let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let w = [
                        cross(v[1].pos - p, v[2].pos - p) / area,
                        cross(v[2].pos - p, v[0].pos - p) / area,
                        cross(v[0].pos - p, v[1].pos - p) / area,
                    ];
                    if w.iter().any(|&w| w < 0.) {
                        continue;
                    }
                    let uv = v[0].uv.to_vec2() * w[0]
                        + v[1].uv.to_vec2() * w[1]
                        + v[2].uv.to_vec2() * w[2];
                    let tx = (uv.x * texture.size[0] as f32)
                        .floor()
                        .clamp(0., (texture.size[0] - 1) as f32)
                        as usize;
                    let ty = (uv.y * texture.size[1] as f32)
                        .floor()
                        .clamp(0., (texture.size[1] - 1) as f32)
                        as usize;
                    let tex = texture.pixels[ty * texture.size[0] + tx].to_array();
                    let mut rgba = [0f32; 4];
                    for channel in 0..4 {
                        rgba[channel] = tex[channel] as f32 / 255.
                            * (0..3)
                                .map(|i| v[i].color[channel] as f32 * w[i])
                                .sum::<f32>();
                    }
                    let dest = canvas.get_pixel_mut(x, y);
                    let alpha = rgba[3] / 255.;
                    for channel in 0..3 {
                        dest[channel] =
                            (rgba[channel] + dest[channel] as f32 * (1. - alpha)).min(255.) as u8;
                    }
                }
            }
        }
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    canvas.save(path).unwrap();
}
