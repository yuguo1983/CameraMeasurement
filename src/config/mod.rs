use anyhow::{Context, Result};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub camera: CameraConfig,
    pub stereo: StereoConfig,
    pub reconstruct: ReconstructConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StereoConfig {
    pub algorithm: String,
    pub block_size: u32,
    pub max_disparity: u32,
    pub min_disparity: i32,
    pub uniqueness: f64,
    pub speckle_window_size: u32,
    pub speckle_range: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReconstructConfig {
    pub depth_min_mm: f64,
    pub depth_max_mm: f64,
    pub median_filter: bool,
    pub downsampling: u32,
}

impl Default for StereoConfig {
    fn default() -> Self {
        Self {
            algorithm: "block_match".to_string(),
            block_size: 9,
            max_disparity: 128,
            min_disparity: 0,
            uniqueness: 0.8,
            speckle_window_size: 100,
            speckle_range: 2,
        }
    }
}

impl Default for ReconstructConfig {
    fn default() -> Self {
        Self {
            depth_min_mm: 100.0,
            depth_max_mm: 5000.0,
            median_filter: true,
            downsampling: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CameraConfig {
    pub resolution: (u32, u32),
    pub baseline_mm: f64,
    pub focal_length_px: f64,
    pub principal_point: (f64, f64),
    pub distortion: CameraDistortion,
    pub exposure_time_ms: u32,
    pub gain: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CameraDistortion {
    pub k1: f64,
    pub k2: f64,
    pub p1: f64,
    pub p2: f64,
    pub k3: f64,
}

impl Default for CameraDistortion {
    fn default() -> Self {
        Self {
            k1: 0.0,
            k2: 0.0,
            p1: 0.0,
            p2: 0.0,
            k3: 0.0,
        }
    }
}

impl CameraDistortion {
    pub fn as_vector(&self) -> Vector3<f64> {
        Vector3::new(self.k1, self.k2, self.p1)
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config from {:?}", path))
    }
}

impl CameraConfig {
    pub fn sensor_size_mm(&self) -> (f64, f64) {
        let (w, h) = self.resolution;
        let sensor_w = w as f64 * self.focal_length_px / self.principal_point.0 * 0.001;
        let sensor_h = h as f64 * self.focal_length_px / self.principal_point.1 * 0.001;
        (sensor_w, sensor_h)
    }

    pub fn scale_factor(&self) -> f64 {
        self.focal_length_px / self.principal_point.0
    }
}