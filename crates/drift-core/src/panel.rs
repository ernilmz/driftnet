//! Screen-independent popup placement. All inputs must use one coordinate space (desktop points on macOS, physical pixels on Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy)]
pub enum Edge {
    Above,
    Below,
}
/// Work area excludes the OS menu bar, dock and taskbar. Negative origins are valid.
pub fn place(
    anchor: Rect,
    work: Rect,
    requested: (u32, u32),
    preferred: Edge,
    gap: u32,
    margin: u32,
) -> Placement {
    let margin = margin.min(work.width / 4).min(work.height / 4) as i64;
    let width = requested
        .0
        .min(work.width.saturating_sub(2 * margin as u32))
        .max(1);
    let height = requested
        .1
        .min(work.height.saturating_sub(2 * margin as u32))
        .max(1);
    let left = work.x as i64 + margin;
    let top = work.y as i64 + margin;
    let right = (work.x as i64 + work.width as i64 - margin - width as i64).max(left);
    let bottom = (work.y as i64 + work.height as i64 - margin - height as i64).max(top);
    let below = anchor.y as i64 + anchor.height as i64 + gap as i64;
    let above = anchor.y as i64 - height as i64 - gap as i64;
    let (first, second) = match preferred {
        Edge::Below => (below, above),
        Edge::Above => (above, below),
    };
    let y = if (top..=bottom).contains(&first) {
        first
    } else if (top..=bottom).contains(&second) {
        second
    } else {
        first.clamp(top, bottom)
    };
    let x = (anchor.x as i64 + anchor.width as i64 / 2 - width as i64 / 2).clamp(left, right);
    Placement {
        x: x as i32,
        y: y as i32,
        width,
        height,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn area(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }
    fn inside(p: Placement, w: Rect) {
        assert!(p.x >= w.x && p.y >= w.y);
        assert!(p.x as i64 + p.width as i64 <= w.x as i64 + w.width as i64);
        assert!(p.y as i64 + p.height as i64 <= w.y as i64 + w.height as i64);
    }
    #[test]
    fn below_menu_bar_avoids_right_edge() {
        let w = area(0, 24, 1440, 840);
        let p = place(area(1390, 0, 24, 24), w, (424, 248), Edge::Below, 6, 8);
        assert_eq!(p.x, 1008);
        assert_eq!(p.y, 32);
        inside(p, w);
    }
    #[test]
    fn negative_origin_display_is_preserved() {
        let w = area(-1920, 0, 1920, 1040);
        let p = place(area(-120, 1040, 24, 40), w, (424, 444), Edge::Above, 6, 8);
        assert!(p.x < 0);
        inside(p, w);
    }
    #[test]
    fn top_taskbar_flips_below() {
        let w = area(0, 40, 1920, 1040);
        let p = place(area(1750, 0, 24, 40), w, (424, 444), Edge::Above, 8, 8);
        assert_eq!(p.y, 48);
        inside(p, w);
    }
    #[test]
    fn work_area_respects_left_and_right_taskbars() {
        for w in [area(60, 0, 1860, 1080), area(0, 0, 1860, 1080)] {
            for a in [area(0, 400, 40, 24), area(1860, 400, 40, 24)] {
                inside(place(a, w, (424, 444), Edge::Above, 8, 8), w);
            }
        }
    }
    #[test]
    fn resizing_settings_stays_on_the_same_monitor() {
        let w = area(1920, 0, 2560, 1400);
        let a = area(4100, 1400, 30, 40);
        let h = place(a, w, (848, 496), Edge::Above, 12, 16);
        let s = place(a, w, (848, 888), Edge::Above, 12, 16);
        assert_eq!(h.x, s.x);
        assert!(s.y < h.y);
        inside(s, w);
    }
    #[test]
    fn small_scaled_display_clamps_size_without_overflow() {
        let w = area(0, 40, 768, 680);
        let p = place(area(700, 0, 40, 40), w, (848, 888), Edge::Below, 12, 16);
        assert_eq!((p.width, p.height), (736, 648));
        inside(p, w);
    }
    #[test]
    fn exhaustive_edges_and_mixed_dpi_sizes_fit() {
        for origin in [-3840, -1920, 0, 1920] {
            let w = area(origin, -400, 1920, 1040);
            for x in [origin - 20, origin, origin + 960, origin + 1910] {
                for y in [-450, -400, 200, 650] {
                    for size in [(424, 248), (530, 310), (636, 666), (848, 888)] {
                        inside(place(area(x, y, 24, 30), w, size, Edge::Below, 8, 8), w);
                        inside(place(area(x, y, 24, 30), w, size, Edge::Above, 8, 8), w);
                    }
                }
            }
        }
    }
}
