use anyhow::Result;
use image::{DynamicImage, GrayImage, ImageBuffer, Luma};
use log::info;
use rayon::prelude::*;

use crate::config::StereoConfig;

pub struct StereoRectified {
    pub disparity: GrayImage,
    pub valid_mask: ImageBuffer<Luma<u8>, Vec<u8>>,
}

pub fn compute_disparity(
    left: &DynamicImage,
    right: &DynamicImage,
    config: &StereoConfig,
) -> Result<GrayImage> {
    let left_gray = left.to_rgb8();
    let right_gray = right.to_rgb8();

    let width = left_gray.width();
    let height = left_gray.height();

    let mut disparity = ImageBuffer::new(width, height);

    let block_half = config.block_size as i32 / 2;

    info!(
        "Computing disparity map: {}x{}, block_size={}, max_disparity={}",
        width, height, config.block_size, config.max_disparity
    );

    let results: Vec<(u32, u32, i32)> = (0..height)
        .into_par_iter()
        .flat_map(|y| {
            let mut row_results = Vec::with_capacity(width as usize);
            for x in 0..width {
                let d = compute_disparity_at(
                    &left_gray,
                    &right_gray,
                    x as i32,
                    y as i32,
                    block_half,
                    config.min_disparity,
                    config.max_disparity as i32,
                    width as i32,
                    height as i32,
                );
                row_results.push((x, y, d));
            }
            row_results
        })
        .collect();

    for (x, y, d) in results {
        disparity.put_pixel(x, y, Luma([d.max(0).min(255) as u8]));
    }

    info!("Disparity computation complete");
    Ok(disparity)
}

fn compute_disparity_at(
    left: &ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    right: &ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    block_half: i32,
    min_disparity: i32,
    max_disparity: i32,
    _width: i32,
    height: i32,
) -> i32 {
    let mut best_disp = min_disparity;
    let mut min_sad = i32::MAX;

    let y_start = (y - block_half).max(0) as u32;
    let y_end = (y + block_half).min(height - 1) as u32;
    let x_start = (x - block_half).max(0) as u32;

    for d in min_disparity..max_disparity {
        let mut sad = 0;
        let mut count = 0;

        let match_x = x + d;

        if match_x < 0 || match_x >= 1024 {
            continue;
        }

        for cy in y_start..=y_end {
            for cx in x_start..=(x + block_half).min(1919) as u32 {
                let ref_pixel = left.get_pixel(cx, cy);
                if let Some(match_pixel) = right.get_pixel_checked(match_x as u32, cy) {
                    sad += (ref_pixel[0] as i32 - match_pixel[0] as i32).abs()
                        + (ref_pixel[1] as i32 - match_pixel[1] as i32).abs()
                        + (ref_pixel[2] as i32 - match_pixel[2] as i32).abs();
                    count += 1;
                }
            }
        }

        if count > 0 {
            sad /= count;
        }

        if sad < min_sad {
            min_sad = sad;
            best_disp = d;
        }
    }

    best_disp
}