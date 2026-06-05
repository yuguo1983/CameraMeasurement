use anyhow::Result;
use image::{GrayImage, Rgb, RgbImage};
use log::{error, info};
use nalgebra::Vector3;
use std::io::Write;
use std::path::PathBuf;

use crate::calibration::{StereoParameters};
use crate::config::ReconstructConfig;

pub type PointCloud = Vec<Vector3<f64>>;

pub struct MeasurementPoint {
    pub image_left: (f64, f64),
    pub image_right: (f64, f64),
    pub point_3d: Vector3<f64>,
}

pub struct MeasurementSession {
    pub params: StereoParameters,
    pub config: ReconstructConfig,
    pub left_image: RgbImage,
    pub right_image: RgbImage,
    pub points: Vec<MeasurementPoint>,
}

impl MeasurementSession {
    pub fn new(
        params: StereoParameters,
        config: ReconstructConfig,
        left_image: RgbImage,
        right_image: RgbImage,
    ) -> Self {
        Self {
            params,
            config,
            left_image,
            right_image,
            points: Vec::new(),
        }
    }

    pub fn add_point(&mut self, left_pt: (f64, f64)) -> Result<Option<Vector3<f64>>> {
        let search_window = 50.0;
        let right_search_x_start = (left_pt.0 - search_window).max(0.0);
        let right_search_x_end = left_pt.0 + search_window;

        let mut best_match = (right_search_x_end, 0.0);
        let mut best_sad = f64::MAX;

        let block_size = 5i32;
        let block_half = block_size / 2;

        for right_x in right_search_x_start as i32..=right_search_x_end as i32 {
            let mut sad = 0.0;
            let mut count = 0;

            for dy in -block_half..=block_half {
                for dx in -block_half..=block_half {
                    let lx = (left_pt.0 as i32 + dx).clamp(0, self.left_image.width() as i32 - 1) as u32;
                    let ly = (left_pt.1 as i32 + dy).clamp(0, self.left_image.height() as i32 - 1) as u32;
                    let rx = (right_x + dx).clamp(0, self.right_image.width() as i32 - 1) as u32;
                    let ry = ly;

                    let lp = self.left_image.get_pixel(lx, ly);
                    let rp = self.right_image.get_pixel(rx, ry);

                    sad += (lp[0] as f64 - rp[0] as f64).abs()
                        + (lp[1] as f64 - rp[1] as f64).abs()
                        + (lp[2] as f64 - rp[2] as f64).abs();
                    count += 1;
                }
            }

            if count > 0 {
                sad /= count as f64;
            }

            if sad < best_sad {
                best_sad = sad;
                best_match.0 = right_x as f64;
                best_match.1 = sad;
            }
        }

        let right_pt = (best_match.0, left_pt.1);

        if let Some(point_3d) = self.params.triangulate(left_pt, right_pt) {
            let measurement = MeasurementPoint {
                image_left: left_pt,
                image_right: right_pt,
                point_3d,
            };
            self.points.push(measurement);
            info!("Added 3D point: ({:.2}, {:.2}, {:.2})", point_3d.x, point_3d.y, point_3d.z);
            return Ok(Some(point_3d));
        }

        Ok(None)
    }

    pub fn measure_distance(&self, idx1: usize, idx2: usize) -> Option<f64> {
        if idx1 >= self.points.len() || idx2 >= self.points.len() {
            return None;
        }
        let p1 = &self.points[idx1].point_3d;
        let p2 = &self.points[idx2].point_3d;
        Some((p1 - p2).norm())
    }

    pub fn draw_points(&self) -> RgbImage {
        let mut display = self.left_image.clone();

        for (i, mp) in self.points.iter().enumerate() {
            let color = match i % 4 {
                0 => Rgb([255, 0, 0]),
                1 => Rgb([0, 255, 0]),
                2 => Rgb([0, 0, 255]),
                _ => Rgb([255, 255, 0]),
            };

            let cx = mp.image_left.0 as i32;
            let cy = mp.image_left.1 as i32;

            for dy in -5..=5 {
                for dx in -5..=5 {
                    let px = (cx + dx).clamp(0, display.width() as i32 - 1) as u32;
                    let py = (cy + dy).clamp(0, display.height() as i32 - 1) as u32;
                    display.put_pixel(px, py, color);
                }
            }
        }

        display
    }
}

pub fn from_disparity(
    disparity: &GrayImage,
    params: &StereoParameters,
    config: &ReconstructConfig,
) -> Result<PointCloud> {
    let width = disparity.width();
    let height = disparity.height();

    info!(
        "Reconstructing point cloud from disparity: {}x{}",
        width, height
    );

    let baseline = params.baseline_mm;
    let fx = params.fx();
    let cx = params.cx();
    let cy = params.cy();

    let mut points = PointCloud::new();

    for y in 0..height {
        for x in 0..width {
            let d = disparity.get_pixel(x, y)[0] as f64;
            if d < 1.0 {
                continue;
            }

            let depth = baseline * fx / d;

            if depth < config.depth_min_mm || depth > config.depth_max_mm {
                continue;
            }

            let x3d = (x as f64 - cx) * depth / fx;
            let y3d = (y as f64 - cy) * depth / fx;
            let z3d = depth;

            points.push(Vector3::new(x3d, y3d, z3d));
        }
    }

    info!("Point cloud generated: {} points", points.len());
    Ok(points)
}

pub fn save_point_cloud(points: &PointCloud, path: &PathBuf) -> Result<()> {
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "ply")?;
    writeln!(file, "format ascii 1.0")?;
    writeln!(file, "element vertex {}", points.len())?;
    writeln!(file, "property float x")?;
    writeln!(file, "property float y")?;
    writeln!(file, "property float z")?;
    writeln!(file, "end_header")?;

    for p in points {
        writeln!(file, "{} {} {}", p.x, p.y, p.z)?;
    }

    Ok(())
}