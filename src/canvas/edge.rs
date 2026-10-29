use eframe::egui::{self, Pos2, Rect};

pub(super) fn visible_edge(
    curve: egui::epaint::CubicBezierShape,
    clip: Rect,
) -> Option<egui::epaint::CubicBezierShape> {
    let mut range = 0.0_f32..1.0_f32;
    for vertical in [true, false] {
        let coordinate = |point: Pos2| if vertical { point.y } else { point.x };
        let values = curve.points.map(coordinate);
        let increasing = values.windows(2).all(|pair| pair[0] <= pair[1]);
        let decreasing = values.windows(2).all(|pair| pair[0] >= pair[1]);
        if !increasing && !decreasing {
            continue;
        }
        let (min, max) = if vertical {
            (clip.top(), clip.bottom())
        } else {
            (clip.left(), clip.right())
        };
        let low = values[0].min(values[3]);
        let high = values[0].max(values[3]);
        if high < min || low > max {
            return None;
        }
        if high - low < 0.001 {
            continue;
        }
        let at = |target: f32| {
            if increasing && target <= values[0] || decreasing && target >= values[0] {
                return 0.0;
            }
            if increasing && target >= values[3] || decreasing && target <= values[3] {
                return 1.0;
            }
            let mut a = 0.0;
            let mut b = 1.0;
            for _ in 0..24 {
                let middle = (a + b) * 0.5;
                if (coordinate(curve.sample(middle)) < target) == increasing {
                    a = middle;
                } else {
                    b = middle;
                }
            }
            (a + b) * 0.5
        };
        let a = at(min);
        let b = at(max);
        range.start = range.start.max(a.min(b));
        range.end = range.end.min(a.max(b));
        if range.start >= range.end {
            return None;
        }
    }
    Some(if range.start == 0.0 && range.end == 1.0 {
        curve
    } else {
        curve.split_range(range)
    })
}
