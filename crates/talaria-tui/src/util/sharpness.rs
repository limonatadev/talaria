#[cfg(not(windows))]
use opencv::core::{self, AlgorithmHint, Mat, Scalar};
#[cfg(not(windows))]
use opencv::imgproc;
#[cfg(not(windows))]
use opencv::prelude::*;

#[cfg(windows)]
use image::RgbImage;

#[cfg(not(windows))]
pub fn laplacian_variance(frame: &Mat) -> anyhow::Result<f64> {
    let mut gray = Mat::default();
    if frame.channels() > 1 {
        imgproc::cvt_color(
            frame,
            &mut gray,
            imgproc::COLOR_BGR2GRAY,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;
    } else {
        gray = frame.clone();
    }

    let mut lap = Mat::default();
    imgproc::laplacian(
        &gray,
        &mut lap,
        core::CV_64F,
        1,
        1.0,
        0.0,
        core::BORDER_DEFAULT,
    )?;

    let mut mean = Scalar::default();
    let mut stddev = Scalar::default();
    core::mean_std_dev(&lap, &mut mean, &mut stddev, &core::no_array())?;
    Ok(stddev[0] * stddev[0])
}

#[cfg(windows)]
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

#[cfg(not(windows))]
#[cfg(test)]
mod tests {
    use super::*;
    use opencv::core::Size;

    #[test]
    fn sharp_image_scores_higher() {
        let mut sharp = Mat::zeros(120, 120, opencv::core::CV_8UC1)
            .expect("mat")
            .to_mat()
            .expect("mat");
        imgproc::rectangle(
            &mut sharp,
            core::Rect::new(20, 20, 80, 80),
            Scalar::new(255.0, 255.0, 255.0, 0.0),
            -1,
            imgproc::LINE_8,
            0,
        )
        .expect("rect");

        let mut blurred = Mat::default();
        imgproc::gaussian_blur(
            &sharp,
            &mut blurred,
            Size::new(9, 9),
            0.0,
            0.0,
            core::BORDER_DEFAULT,
            core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )
        .expect("blur");

        let sharp_score = laplacian_variance(&sharp).expect("score");
        let blur_score = laplacian_variance(&blurred).expect("score");
        assert!(sharp_score > blur_score);
    }
}
