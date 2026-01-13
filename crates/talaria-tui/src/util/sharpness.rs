use image::RgbImage;

pub fn laplacian_variance(frame: &RgbImage) -> anyhow::Result<f64> {
    let (width, height) = frame.dimensions();
    if width < 3 || height < 3 {
        return Ok(0.0);
    }

    let gray_at = |x: u32, y: u32| -> f64 {
        let px = frame.get_pixel(x, y);
        0.299 * px[0] as f64 + 0.587 * px[1] as f64 + 0.114 * px[2] as f64
    };

    let mut values = Vec::with_capacity(((width - 2) * (height - 2)) as usize);
    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let center = gray_at(x, y);
            let lap = -4.0 * center
                + gray_at(x - 1, y)
                + gray_at(x + 1, y)
                + gray_at(x, y - 1)
                + gray_at(x, y + 1);
            values.push(lap);
        }
    }

    let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
    let variance = values
        .iter()
        .map(|v| {
            let d = v - mean;
            d * d
        })
        .sum::<f64>()
        / values.len().max(1) as f64;
    Ok(variance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn sharp_image_scores_higher() {
        let mut sharp = RgbImage::from_pixel(120, 120, Rgb([0, 0, 0]));
        for y in 20..100 {
            for x in 20..100 {
                sharp.put_pixel(x, y, Rgb([255, 255, 255]));
            }
        }

        let mut blurred = RgbImage::from_pixel(120, 120, Rgb([0, 0, 0]));
        for y in 20..100 {
            for x in 20..100 {
                let value = if (x + y) % 2 == 0 { 200 } else { 55 };
                blurred.put_pixel(x, y, Rgb([value, value, value]));
            }
        }

        let sharp_score = laplacian_variance(&sharp).expect("score");
        let blur_score = laplacian_variance(&blurred).expect("score");
        assert!(sharp_score > blur_score);
    }
}
