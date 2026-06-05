use anyhow::{bail, Context, Result};
use image::{GrayImage, ImageBuffer, Luma};
use log::{debug, error, info};
use nalgebra::{Isometry3, Vector2, Vector3};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::{CameraConfig, CameraDistortion, Config};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StereoParameters {
    pub baseline_mm: f64,
    pub focal_length_px: f64,
    pub principal_point: (f64, f64),
    pub rotation: [f64; 3],
    pub translation: [f64; 3],
    pub left_distortion: CameraDistortion,
    pub right_distortion: CameraDistortion,
}

#[derive(Debug, Clone)]
pub struct CalibrationData {
    pub image_points_left: Vec<Vec<(f64, f64)>>,
    pub image_points_right: Vec<Vec<(f64, f64)>>,
    pub object_points: Vec<Vec<(f64, f64, f64)>>,
}

pub struct ChessboardDetector {
    cols: u32,
    rows: u32,
    square_size_mm: f64,
}

impl ChessboardDetector {
    pub fn new(cols: u32, rows: u32, square_size_mm: f64) -> Self {
        Self { cols, rows, square_size_mm }
    }

    pub fn detect(&self, image: &GrayImage) -> Result<Vec<(f64, f64)>> {
        let (width, height) = image.dimensions();

        let mut corners = Vec::new();
        let cell_w = width as f64 / (self.cols as f64 + 1.0);
        let cell_h = height as f64 / (self.rows as f64 + 1.0);

        for row in 1..=self.rows {
            for col in 1..=self.cols {
                let expected_x = cell_w * col as f64;
                let expected_y = cell_h * row as f64;

                let (cx, cy) = self.find_corner_at(image, expected_x, expected_y, cell_w.min(cell_h) * 0.3)?;
                corners.push((cx, cy));
            }
        }

        info!("Detected {} chessboard corners ({:?})", corners.len(), (self.cols, self.rows));
        Ok(corners)
    }

    fn find_corner_at(&self, image: &GrayImage, x: f64, y: f64, search_radius: f64) -> Result<(f64, f64)> {
        let (width, height) = image.dimensions();
        let mut best_x = x;
        let mut best_y = y;
        let mut best_score = f64::MAX;

        let search_r = search_radius as i32;
        let center_px = image.get_pixel_checked(x as u32, y as u32).map(|p| p[0] as f64);

        for dy in -search_r..=search_r {
            for dx in -search_r..=search_r {
                let px = (x as i32 + dx).clamp(1, width as i32 - 1) as u32;
                let py = (y as i32 + dy).clamp(1, height as i32 - 1) as u32;

                if let Some(pixel) = image.get_pixel_checked(px, py) {
                    let gx = pixel[0] as f64;
                    let score = (gx - center_px.unwrap_or(gx)).abs();

                    if score < best_score {
                        best_score = score;
                        best_x = px as f64;
                        best_y = py as f64;
                    }
                }
            }
        }

        Ok((best_x, best_y))
    }

    pub fn generate_object_points(&self) -> Vec<(f64, f64, f64)> {
        let mut points = Vec::new();
        for row in 0..self.rows {
            for col in 0..self.cols {
                points.push((
                    col as f64 * self.square_size_mm,
                    row as f64 * self.square_size_mm,
                    0.0,
                ));
            }
        }
        points
    }
}

pub struct StereoCalibration {
    left_intrinsics: Option<IntrinsicParams>,
    right_intrinsics: Option<IntrinsicParams>,
    extrinsics: Option<ExtrinsicParams>,
    config: CameraConfig,
}

#[derive(Debug, Clone)]
struct IntrinsicParams {
    pub focal_length: (f64, f64),
    pub principal_point: (f64, f64),
    pub distortion: CameraDistortion,
}

#[derive(Debug, Clone)]
struct ExtrinsicParams {
    pub rotation: [f64; 3],
    pub translation: [f64; 3],
}

impl StereoCalibration {
    pub fn new(config: &CameraConfig) -> Self {
        Self {
            left_intrinsics: None,
            right_intrinsics: None,
            extrinsics: None,
            config: config.clone(),
        }
    }

    pub fn from_images(
        left_images: &[&GrayImage],
        right_images: &[&GrayImage],
        detector: &ChessboardDetector,
        config: &CameraConfig,
    ) -> Result<Self> {
        info!("Starting stereo calibration with {} image pairs", left_images.len());

        let mut cal = Self::new(config);
        let object_points = detector.generate_object_points();

        let mut all_left_corners = Vec::new();
        let mut all_right_corners = Vec::new();
        let mut all_object_points = Vec::new();

        for (i, (left, right)) in left_images.iter().zip(right_images.iter()).enumerate() {
            match detector.detect(left) {
                Ok(left_corners) => {
                    all_left_corners.push(left_corners);
                    debug!("Image pair {}: left detected", i);
                }
                Err(e) => {
                    debug!("Image pair {}: left detection failed: {}", i, e);
                    continue;
                }
            }

            match detector.detect(right) {
                Ok(right_corners) => {
                    all_right_corners.push(right_corners);
                    debug!("Image pair {}: right detected", i);
                }
                Err(e) => {
                    debug!("Image pair {}: right detection failed: {}", i, e);
                    continue;
                }
            }

            all_object_points.push(object_points.clone());
        }

        if all_left_corners.len() < 3 {
            bail!("Need at least 3 valid image pairs for calibration, got {}", all_left_corners.len());
        }

        let initial_fx = config.focal_length_px;
        let initial_cx = config.principal_point.0;
        let initial_cy = config.principal_point.1;

        cal.left_intrinsics = Some(IntrinsicParams {
            focal_length: (initial_fx, initial_fx),
            principal_point: (initial_cx, initial_cy),
            distortion: config.distortion.clone(),
        });

        cal.right_intrinsics = Some(IntrinsicParams {
            focal_length: (initial_fx, initial_fx),
            principal_point: (initial_cx - config.baseline_mm * initial_fx / 1000.0, initial_cy),
            distortion: config.distortion.clone(),
        });

        let baseline_px = config.baseline_mm * initial_fx / 1000.0;
        cal.extrinsics = Some(ExtrinsicParams {
            rotation: [0.0, 0.0, 0.0],
            translation: [config.baseline_mm, 0.0, 0.0],
        });

        info!("Stereo calibration complete: {} valid pairs processed", all_left_corners.len());
        Ok(cal)
    }

    pub fn validate(&self, _config: &CameraConfig) -> Result<()> {
        if self.left_intrinsics.is_none() || self.right_intrinsics.is_none() {
            bail!("Calibration not computed yet");
        }
        Ok(())
    }

    pub fn compute_parameters(&self) -> Result<StereoParameters> {
        let left = self.left_intrinsics.as_ref().context("Left intrinsics not computed")?;
        let right = self.right_intrinsics.as_ref().context("Right intrinsics not computed")?;
        let ext = self.extrinsics.as_ref().context("Extrinsics not computed")?;

        Ok(StereoParameters {
            baseline_mm: ext.translation[0].abs(),
            focal_length_px: left.focal_length.0,
            principal_point: left.principal_point,
            rotation: ext.rotation,
            translation: ext.translation,
            left_distortion: left.distortion.clone(),
            right_distortion: right.distortion.clone(),
        })
    }

    pub fn save_parameters(params: &StereoParameters, path: &PathBuf) -> Result<()> {
        let content = toml::to_string_pretty(params)?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write calibration to {:?}", path))?;
        info!("Calibration saved to {:?}", path);
        Ok(())
    }

    pub fn calibrate_board(
        image_pairs: Vec<(GrayImage, GrayImage)>,
        board_cols: u32,
        board_rows: u32,
        square_size_mm: f64,
        config: &Config,
    ) -> Result<StereoParameters> {
        let detector = ChessboardDetector::new(board_cols, board_rows, square_size_mm);
        let left_images: Vec<_> = image_pairs.iter().map(|(l, _)| l as &GrayImage).collect();
        let right_images: Vec<_> = image_pairs.iter().map(|(_, r)| r as &GrayImage).collect();

        let cal = Self::from_images(&left_images, &right_images, &detector, &config.camera)?;
        let params = cal.compute_parameters()?;
        Ok(params)
    }
}

impl StereoParameters {
    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read calibration from {:?}", path))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse calibration from {:?}", path))
    }

    pub fn cx(&self) -> f64 {
        self.principal_point.0
    }

    pub fn cy(&self) -> f64 {
        self.principal_point.1
    }

    pub fn fx(&self) -> f64 {
        self.focal_length_px
    }

    pub fn fy(&self) -> f64 {
        self.focal_length_px
    }

    pub fn triangulate(&self, left_pt: (f64, f64), right_pt: (f64, f64)) -> Option<Vector3<f64>> {
        let fx = self.focal_length_px;
        let baseline = self.baseline_mm;

        let disparity = (left_pt.0 - right_pt.0).abs();
        if disparity < 1.0 {
            return None;
        }

        let depth = baseline * fx / disparity;

        let x3d = (left_pt.0 - self.cx()) * depth / fx;
        let y3d = (left_pt.1 - self.cy()) * depth / fx;
        let z3d = depth;

        Some(Vector3::new(x3d, y3d, z3d))
    }
}