use opencv::core::{self, AlgorithmHint, Mat, Scalar};
use opencv::imgproc;
use opencv::prelude::*;

pub fn laplacian_variance(frame: &Mat) -> opencv::Result<f64> {
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
