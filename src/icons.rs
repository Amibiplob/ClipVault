use eframe::egui::*;
pub fn draw(p: &Painter, r: Rect, name: &str, color: Color32) {
    let c = r.center();
    let z = r.width().min(r.height()) / 24.;
    let point = |x: f32, y: f32| c + vec2((x - 12.) * z, (y - 12.) * z);
    let stroke = Stroke::new(1.55 * z, color);
    let line = |v: &[(f32, f32)]| {
        p.add(Shape::line(
            v.iter().map(|&(x, y)| point(x, y)).collect(),
            stroke,
        ));
    };
    let rect = |x, y, w, h, rad| {
        p.rect_stroke(
            Rect::from_min_size(point(x, y), vec2(w * z, h * z)),
            rad,
            stroke,
            StrokeKind::Inside,
        );
    };
    match name {
        "clipboard" => {
            rect(5., 5., 14., 16., 2.);
            rect(9., 2., 6., 5., 1.);
            line(&[(9., 12.), (15., 12.)]);
            line(&[(9., 16.), (14., 16.)]);
        }
        "search" => {
            p.circle_stroke(point(10.5, 10.5), 6.5 * z, stroke);
            line(&[(15.5, 15.5), (21., 21.)]);
        }
        "settings" => {
            for n in 0..8 {
                let a = n as f32 * std::f32::consts::TAU / 8.;
                let v = vec2(a.cos(), a.sin());
                p.line_segment([c + v * 7. * z, c + v * 9.5 * z], stroke);
            }
            p.circle_stroke(c, 7. * z, stroke);
            p.circle_stroke(c, 2.5 * z, stroke);
        }
        "pin" => {
            line(&[(8., 3.), (16., 3.)]);
            line(&[
                (9., 3.),
                (9., 9.),
                (6., 13.),
                (18., 13.),
                (15., 9.),
                (15., 3.),
            ]);
            line(&[(12., 13.), (12., 21.)]);
        }
        "image" => {
            rect(3., 3., 18., 18., 2.);
            p.circle_stroke(point(8., 8.), 1.5 * z, stroke);
            line(&[(3., 17.), (9., 11.), (13., 15.), (16., 12.), (21., 17.)]);
        }
        "file" => {
            line(&[
                (14., 2.),
                (5., 2.),
                (5., 22.),
                (19., 22.),
                (19., 7.),
                (14., 2.),
                (14., 8.),
                (19., 8.),
            ]);
            line(&[(9., 13.), (15., 13.)]);
            line(&[(9., 17.), (14., 17.)]);
        }
        "link" => {
            line(&[(9., 15.), (15., 9.)]);
            line(&[
                (8., 17.),
                (6., 19.),
                (3., 19.),
                (1., 17.),
                (1., 14.),
                (7., 8.),
                (10., 8.),
                (12., 10.),
            ]);
            line(&[
                (12., 14.),
                (14., 16.),
                (17., 16.),
                (23., 10.),
                (23., 7.),
                (21., 5.),
                (18., 5.),
                (16., 7.),
            ]);
        }
        "text" => {
            line(&[(5., 5.), (19., 5.)]);
            line(&[(12., 5.), (12., 20.)]);
            line(&[(8., 20.), (16., 20.)]);
        }
        "code" => {
            line(&[(8., 6.), (2., 12.), (8., 18.)]);
            line(&[(16., 6.), (22., 12.), (16., 18.)]);
            line(&[(14., 3.), (10., 21.)]);
        }
        "close" => {
            line(&[(6., 6.), (18., 18.)]);
            line(&[(18., 6.), (6., 18.)]);
        }
        "arrow" => {
            line(&[(5., 12.), (19., 12.), (14., 7.)]);
            line(&[(19., 12.), (14., 17.)]);
        }
        "enter" => {
            line(&[(19., 5.), (19., 14.), (5., 14.), (10., 9.)]);
            line(&[(5., 14.), (10., 19.)]);
        }
        "up" => {
            line(&[(6., 15.), (12., 9.), (18., 15.)]);
        }
        "down" => {
            line(&[(6., 9.), (12., 15.), (18., 9.)]);
        }
        "trash" => {
            line(&[(4., 6.), (20., 6.)]);
            line(&[(9., 6.), (9., 3.), (15., 3.), (15., 6.)]);
            line(&[(6., 6.), (7., 21.), (17., 21.), (18., 6.)]);
            line(&[(10., 10.), (10., 17.)]);
            line(&[(14., 10.), (14., 17.)]);
        }
        "pause" => {
            line(&[(8., 5.), (8., 19.)]);
            line(&[(16., 5.), (16., 19.)]);
        }
        "play" => {
            line(&[(7., 4.), (20., 12.), (7., 20.), (7., 4.)]);
        }
        "shield" => {
            line(&[
                (12., 2.),
                (21., 6.),
                (20., 15.),
                (17., 19.),
                (12., 22.),
                (7., 19.),
                (4., 15.),
                (3., 6.),
                (12., 2.),
            ]);
            line(&[(8., 12.), (11., 15.), (16., 9.)]);
        }
        "check" => {
            line(&[(5., 12.), (10., 17.), (20., 6.)]);
        }
        "grid" => {
            rect(3., 3., 7., 7., 1.);
            rect(14., 3., 7., 7., 1.);
            rect(3., 14., 7., 7., 1.);
            rect(14., 14., 7., 7., 1.);
        }
        "dots" => {
            for x in [5., 12., 19.] {
                p.circle_filled(point(x, 12.), 1.3 * z, color);
            }
        }
        "moon" => {
            p.circle_stroke(c, 8. * z, stroke);
        }
        "clock" => {
            p.circle_stroke(c, 9. * z, stroke);
            line(&[(12., 6.), (12., 12.), (16., 14.)]);
        }
        "folder" => {
            line(&[
                (3., 7.),
                (3., 20.),
                (21., 20.),
                (21., 6.),
                (12., 6.),
                (10., 3.),
                (3., 3.),
                (3., 7.),
            ]);
        }
        "keyboard" => {
            rect(2., 5., 20., 14., 2.);
            for y in [9., 12.] {
                for x in [6., 10., 14., 18.] {
                    p.circle_filled(point(x, y), 0.6 * z, color);
                }
            }
            line(&[(8., 16.), (16., 16.)]);
        }
        _ => {
            p.circle_stroke(c, 8. * z, stroke);
            line(&[(12., 10.), (12., 16.)]);
            p.circle_filled(point(12., 6.), 1. * z, color);
        }
    }
}
pub fn button(ui: &mut Ui, name: &str, tip: &str, size: f32, color: Color32) -> Response {
    let (r, res) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if res.hovered() {
        ui.painter()
            .rect_filled(r, 6, Color32::from_white_alpha(13));
    }
    draw(ui.painter(), r.shrink((size - 18.) / 2.), name, color);
    res.on_hover_text(tip)
}
