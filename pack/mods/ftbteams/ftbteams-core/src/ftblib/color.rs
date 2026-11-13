//! `Color4I` as the FTB mods store and parse it: opaque `#RRGGBB`, `#AARRGGBB` with alpha.

/// An ARGB colour.
pub type Argb = u32;

pub const WHITE: Argb = 0xFFFF_FFFF;

/// `Color4I.toString`: `#RRGGBB` (upper case) when opaque, else `#AARRGGBB`.
pub fn to_string(color: Argb) -> String {
    if color >> 24 == 0xFF { format!("#{:06X}", color & 0xFF_FFFF) } else { format!("#{color:08X}") }
}

/// `Color4I.fromString`: `#RRGGBB` (opaque) or `#AARRGGBB`; `transparent`.
pub fn parse(text: &str) -> Option<Argb> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("transparent") {
        return Some(0);
    }
    let hex = text.strip_prefix('#')?;
    let value = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some(0xFF00_0000 | value),
        8 => Some(value),
        _ => None,
    }
}

/// `Color4I.hsb(h, s, b)`: `java.awt.Color.HSBtoRGB`, opaque.
pub fn hsb(hue: f32, saturation: f32, brightness: f32) -> Argb {
    let (r, g, b);
    if saturation == 0.0 {
        let v = (brightness * 255.0 + 0.5) as u32;
        (r, g, b) = (v, v, v);
    } else {
        let h = (hue - hue.floor()) * 6.0;
        let f = h - h.floor();
        let p = brightness * (1.0 - saturation);
        let q = brightness * (1.0 - saturation * f);
        let t = brightness * (1.0 - saturation * (1.0 - f));
        let c = |v: f32| (v * 255.0 + 0.5) as u32;
        (r, g, b) = match h as i32 {
            0 => (c(brightness), c(t), c(p)),
            1 => (c(q), c(brightness), c(p)),
            2 => (c(p), c(brightness), c(t)),
            3 => (c(p), c(q), c(brightness)),
            4 => (c(t), c(p), c(brightness)),
            5 => (c(brightness), c(p), c(q)),
            _ => (0, 0, 0),
        };
    }
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

/// A new team's colour (`FTBTUtils.randomColor`): a random hue at saturation 0.65 and full brightness.
pub fn random_team_color(unit: f32) -> Argb {
    hsb(unit, 0.65, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings() {
        assert_eq!(to_string(0xFF12_34AB), "#1234AB");
        assert_eq!(to_string(0x8012_34AB), "#801234AB");
        assert_eq!(parse("#1234ab"), Some(0xFF12_34AB));
        assert_eq!(parse("#801234AB"), Some(0x8012_34AB));
        assert_eq!(parse("1234AB"), None);
        assert_eq!(parse("#12"), None);
    }

    #[test]
    fn hsb_is_awts() {
        // Color.HSBtoRGB(0, 0.65f, 1) = 0xFFFF5959; (0.5, 0.65, 1) = 0xFF59FFFF.
        assert_eq!(hsb(0.0, 0.65, 1.0), 0xFFFF_5959);
        assert_eq!(hsb(0.5, 0.65, 1.0), 0xFF59_FFFF);
    }
}
