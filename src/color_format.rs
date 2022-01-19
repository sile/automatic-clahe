pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = usize::from(r);
    let g = usize::from(g);
    let b = usize::from(b);
    let max = std::cmp::max(r, std::cmp::max(g, b));
    let min = std::cmp::min(r, std::cmp::min(g, b));
    let n = max - min;

    let s = if max == 0 { 0 } else { n * 255 / max };
    let v = max;
    let h = if n == 0 {
        0
    } else if max == r {
        if g < b {
            (6 * 255) + (g * 255 / n) - (b * 255 / n)
        } else {
            (g - b) * 255 / n
        }
    } else if max == g {
        2 * 255 + b * 255 / n - r * 255 / n
    } else {
        4 * 255 + r * 255 / n - g * 255 / n
    } / 6;

    (h as u8, s as u8, v as u8)
}

pub fn hsv_to_rgb(h: u8, s: u8, v: u8) -> (u8, u8, u8) {
    if s == 0 {
        return (v, v, v);
    }

    let mut r = usize::from(v);
    let mut g = usize::from(v);
    let mut b = usize::from(v);
    let s = usize::from(s);
    let h6 = usize::from(h) * 6;

    let f = h6 % 255;
    match h6 / 255 {
        1 => {
            r = r * (255 * 255 - s * f) / (255 * 255);
            b = b * (255 - s) / 255;
        }
        2 => {
            r = r * (255 - s) / 255;
            b = b * (255 * 255 - s * (255 - f)) / (255 * 255);
        }
        3 => {
            r = r * (255 - s) / 255;
            g = g * (255 * 255 - s * f) / (255 * 255);
        }
        4 => {
            r = r * (255 * 255 - s * (255 - f)) / (255 * 255);
            g = g * (255 - s) / 255;
        }
        5 => {
            g = g * (255 - s) / 255;
            b = b * (255 * 255 - s * f) / (255 * 255);
        }
        n => {
            debug_assert!(n == 0 || n == 6, "n: {}", n);
            g = g * (255 * 255 - s * (255 - f)) / (255 * 255);
            b = b * (255 - s) / 255;
        }
    }

    (r as u8, g as u8, b as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_to_hsv_works() {
        let inputs = [(255, 0, 0), (10, 30, 200), (222, 222, 222)];
        for i in inputs {
            let (h, s, v) = rgb_to_hsv(i.0, i.1, i.2);
            let (r, g, b) = hsv_to_rgb(h, s, v);

            dbg!(i);
            dbg!((r, g, b));

            assert!((i32::from(r) - i32::from(i.0)).abs() <= 2);
            assert!((i32::from(g) - i32::from(i.1)).abs() <= 2);
            assert!((i32::from(b) - i32::from(i.2)).abs() <= 2);
        }
    }
}
