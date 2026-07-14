use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;

const BADGE_FONT: &[u8] = include_bytes!("../../assets/badge-font.ttf");

pub(crate) fn draw_badge(mut image: RgbImage, text: &str) -> RgbImage {
    let font = match FontRef::try_from_slice(BADGE_FONT) {
        Ok(font) => font,

        Err(_) => return image,
    };

    let height = image.height() as f32;
    let band_height = height * 0.22;
    let is_dark_background = average_luminance_is_dark(&image, band_height as u32);

    let (text_color, outline_color) = if is_dark_background {
        (Rgb([255u8, 255, 255]), Rgb([0u8, 0, 0]))
    } else {
        (Rgb([0u8, 0, 0]), Rgb([255u8, 255, 255]))
    };

    let scale = PxScale::from(band_height * 0.6);
    let scaled = font.as_scaled(scale);
    let text_width: f32 = text
        .chars()
        .map(|c| scaled.h_advance(scaled.font.glyph_id(c)))
        .sum();
    let text_height = scaled.ascent() - scaled.descent();
    let x = ((image.width() as f32 - text_width) / 2.0).max(0.0) as i32;
    let y = ((band_height - text_height) / 2.0).max(0.0) as i32;

    let outline_width = ((band_height * 0.04).round() as i32).max(1);
    for dx in [-outline_width, outline_width] {
        for dy in [-outline_width, outline_width] {
            draw_text_mut(
                &mut image,
                outline_color,
                x + dx,
                y + dy,
                scale,
                &font,
                text,
            );
        }
    }
    draw_text_mut(&mut image, text_color, x, y, scale, &font, text);
    image
}

fn average_luminance_is_dark(image: &RgbImage, band_height: u32) -> bool {
    let band_height = band_height.max(1).min(image.height());
    let mut sum = 0f64;
    let mut count = 0f64;
    for y in 0..band_height {
        for x in 0..image.width() {
            let Rgb([r, g, b]) = *image.get_pixel(x, y);
            sum += 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            count += 1.0;
        }
    }
    if count == 0.0 {
        return true;
    }
    (sum / count) < 140.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_changes_pixels_in_top_band_only() {
        let plain = RgbImage::from_pixel(128, 128, Rgb([200, 200, 200]));
        let badged = draw_badge(plain.clone(), "#12");

        let top_band_changed = (0..28u32)
            .any(|y| (0..128u32).any(|x| badged.get_pixel(x, y) != plain.get_pixel(x, y)));
        assert!(
            top_band_changed,
            "le badge doit être visible dans la bande haute"
        );

        let bottom_untouched = (64..128u32)
            .all(|y| (0..128u32).all(|x| badged.get_pixel(x, y) == plain.get_pixel(x, y)));
        assert!(
            bottom_untouched,
            "le bas de l'image ne doit pas être modifié"
        );
    }

    #[test]
    fn text_color_adapts_to_background_luminance() {
        let light = draw_badge(RgbImage::from_pixel(128, 128, Rgb([250, 250, 250])), "#1");
        let has_dark_pixel =
            (0..28u32).any(|y| (0..128u32).any(|x| light.get_pixel(x, y).0[0] < 60));
        assert!(has_dark_pixel, "sur fond clair, le texte doit être sombre");

        let dark = draw_badge(RgbImage::from_pixel(128, 128, Rgb([10, 10, 10])), "#1");
        let has_light_pixel =
            (0..28u32).any(|y| (0..128u32).any(|x| dark.get_pixel(x, y).0[0] > 200));
        assert!(has_light_pixel, "sur fond sombre, le texte doit être clair");
    }
}
