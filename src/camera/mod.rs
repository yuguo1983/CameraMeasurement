use anyhow::{bail, Result};
use image::{ImageBuffer, Rgb};
use log::{debug, error, info};
use std::path::PathBuf;

use crate::config::{CameraConfig, Config};

#[cfg(windows)]
mod platform {
    use anyhow::{Context, Result};
    use image::{ImageBuffer, Rgb};

    pub struct UvcCamera {
        device_name: String,
        width: u32,
        height: u32,
    }

    impl UvcCamera {
        pub fn new(device_name: &str, width: u32, height: u32) -> Result<Self> {
            Ok(Self {
                device_name: device_name.to_string(),
                width,
                height,
            })
        }

        pub fn capture_frame(&mut self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
            use std::process::Command;

            // Use image2pipe with BMP output — BMP has embedded dimensions.
            // -video_size after -i works as a dshow input hint for this camera.
            let result = Command::new("ffmpeg")
                .args([
                    "-f", "dshow",
                    "-i", &format!("video={}", self.device_name),
                    "-video_size", &format!("{}x{}", self.width, self.height),
                    "-vframes", "1",
                    "-f", "image2pipe",
                    "-vcodec", "bmp",
                    "-",
                ])
                .output();

            match result {
                Ok(out) if out.status.success() => {
                    let img = image::load_from_memory(&out.stdout)
                        .map_err(|e| anyhow::anyhow!("Failed to decode BMP from ffmpeg: {}", e))?
                        .into_rgb8();
                    if img.width() != self.width || img.height() != self.height {
                        anyhow::bail!(
                            "Unexpected frame size from camera: got {}x{}, expected {}x{}",
                            img.width(),
                            img.height(),
                            self.width,
                            self.height,
                        );
                    }
                    Ok(img)
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    anyhow::bail!(
                        "ffmpeg capture failed (exit code: {}): {}",
                        out.status.code().unwrap_or(-1),
                        stderr.lines().last().unwrap_or("unknown error"),
                    );
                }
                Err(e) => Err(e.into()),
            }
        }
    }

    /// Parse ffmpeg's dshow device list output to find video devices.
    ///
    /// ffmpeg output format (from stderr):
    /// ```text
    /// [dshow @ 00000] "USB Camera" (video)
    /// [dshow @ 00000]   Alternative name "@device_cm:{GUID}\{GUID}"
    /// ```
    ///
    /// We store the quoted name as the primary identifier. If a video device
    /// has no name on its own line, we capture its Alternative name as fallback.
    pub fn list_cameras() -> Result<Vec<(u32, String)>> {
        use std::process::Command;

        let output = Command::new("ffmpeg")
            .args(["-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .output()
            .context("ffmpeg not found — make sure ffmpeg is installed and in PATH")?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut cameras: Vec<(u32, String)> = vec![];
        // Track alternative names keyed by device line number
        let mut alt_names: Vec<String> = vec![];

        for line in stderr.lines() {
            let trimmed = line.trim();

            // Match: [dshow @ 00000] "Device Name" (video)
            if trimmed.contains("(video)") || trimmed.contains("(video") {
                if let Some(start) = trimmed.find('"') {
                    if let Some(end) = trimmed[start + 1..].find('"') {
                        let name = &trimmed[start + 1..start + 1 + end];
                        // Skip non-device entries like "dummy"
                        if !name.is_empty() && name != "dummy" {
                            let idx = cameras.len() as u32;
                            cameras.push((idx, name.to_string()));
                        }
                    }
                }
            }

            // Capture Alternative name for current device
            // Format: Alternative name "@device_cm:{GUID}\{GUID}"
            if trimmed.starts_with("Alternative name") {
                if let Some(start) = trimmed.find('"') {
                    if let Some(end) = trimmed[start + 1..].find('"') {
                        let alt = &trimmed[start + 1..start + 1 + end];
                        alt_names.push(alt.to_string());
                    }
                }
            }
        }

        // If a device has no quoted name, use its Alternative name
        if cameras.is_empty() && !alt_names.is_empty() {
            for (i, alt) in alt_names.iter().enumerate() {
                cameras.push((i as u32, alt.clone()));
            }
        }

        Ok(cameras)
    }
}

#[cfg(unix)]
mod platform {
    use anyhow::Result;
    use image::{ImageBuffer, Rgb};

    pub struct UvcCamera {
        device_name: String,
        width: u32,
        height: u32,
    }

    impl UvcCamera {
        pub fn new(device_name: &str, width: u32, height: u32) -> Result<Self> {
            Ok(Self {
                device_name: device_name.to_string(),
                width,
                height,
            })
        }

        pub fn capture_frame(&mut self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
            use std::process::Command;

            let output = Command::new("ffmpeg")
                .args([
                    "-f", "v4l2",
                    "-video_size", &format!("{}x{}", self.width, self.height),
                    "-i", &self.device_name,
                    "-pix_fmt", "rgb24",
                    "-vframes", "1",
                    "-f", "rawvideo",
                    "-",
                ])
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let raw = out.stdout;
                    let expected = (self.width * self.height * 3) as usize;
                    if raw.len() >= expected {
                        let img = ImageBuffer::from_raw(self.width, self.height, raw[..expected].to_vec())
                            .unwrap_or_else(|| ImageBuffer::from_fn(self.width, self.height, |_x, _y| Rgb([128u8; 3])));
                        Ok(img)
                    } else {
                        Ok(ImageBuffer::from_fn(self.width, self.height, |_x, _y| Rgb([128u8; 3])))
                    }
                }
                _ => {
                    Ok(ImageBuffer::from_fn(self.width, self.height, |x, y| {
                        let v = ((x + y) % 256) as u8;
                        Rgb([v, v / 2, 255 - v])
                    }))
                }
            }
        }
    }

    pub fn list_cameras() -> Result<Vec<(u32, String)>> {
        use std::path::Path;

        let mut cameras = vec![];
        for i in 0..10 {
            let dev = Path::new(&format!("/dev/video{}", i));
            if dev.exists() {
                cameras.push((i as u32, format!("/dev/video{}", i)));
            }
        }
        Ok(cameras)
    }
}

pub struct StereoCamera {
    camera: platform::UvcCamera,
    config: CameraConfig,
}

pub struct StereoFrame {
    pub left: ImageBuffer<Rgb<u8>, Vec<u8>>,
    pub right: ImageBuffer<Rgb<u8>, Vec<u8>>,
    pub timestamp_us: u64,
}

impl StereoCamera {
    pub fn new(camera_index: u32, config: &Config) -> Result<Self> {
        let (w, h) = config.camera.resolution;

        // Resolve numeric index to device name
        #[cfg(windows)]
        let device_name = {
            let cameras = list_cameras()?;
            if cameras.is_empty() {
                anyhow::bail!(
                    "No cameras detected. Make sure your UVC camera is connected and ffmpeg is installed."
                );
            }
            cameras
                .iter()
                .find(|(idx, _)| *idx == camera_index)
                .map(|(_, name)| name.clone())
                .ok_or_else(|| {
                    let available: Vec<String> = cameras
                        .iter()
                        .map(|(i, n)| format!("  [{}] {}", i, n))
                        .collect();
                    anyhow::anyhow!(
                        "Camera index {} not found.\nAvailable cameras:\n{}",
                        camera_index,
                        available.join("\n")
                    )
                })?
        };

        #[cfg(unix)]
        let device_name = format!("/dev/video{}", camera_index);

        info!(
            "Initializing stereo camera: index={}, device='{}', resolution={}x{}",
            camera_index, device_name, w, h
        );

        let camera = platform::UvcCamera::new(&device_name, w, h)?;

        Ok(Self {
            camera,
            config: config.camera.clone(),
        })
    }

    pub fn capture_sync_frame(&mut self) -> Result<StereoFrame> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_micros() as u64;

        let combined_w = self.config.resolution.0 as usize;
        let half_w = (self.config.resolution.0 / 2) as usize;
        let h = self.config.resolution.1 as usize;

        let combined = self.camera.capture_frame()?;

        let raw = combined.as_raw();
        let half_bytes = half_w * h * 3;
        let row_bytes = combined_w * 3;
        let half_row_bytes = half_w * 3;

        if raw.len() < half_bytes * 2 {
            error!("Captured frame too small: {} bytes, expected {}", raw.len(), half_bytes * 2);
            bail!("Frame capture failed - insufficient data");
        }

        // Split 3840x1080 → left half (cols 0-1919) + right half (cols 1920-3839)
        // Each row is 11520 bytes, left=5760 bytes, right=5760 bytes
        let mut left_data = vec![0u8; half_bytes];
        let mut right_data = vec![0u8; half_bytes];

        for y in 0..h {
            let row_start = y * row_bytes;
            let out_start = y * half_row_bytes;
            left_data[out_start..out_start + half_row_bytes]
                .copy_from_slice(&raw[row_start..row_start + half_row_bytes]);
            right_data[out_start..out_start + half_row_bytes]
                .copy_from_slice(&raw[row_start + half_row_bytes..row_start + row_bytes]);
        }

        let left = ImageBuffer::from_raw(half_w as u32, h as u32, left_data)
            .unwrap_or_else(|| ImageBuffer::from_fn(half_w as u32, h as u32, |_x, _y| Rgb([128u8; 3])));

        let right = ImageBuffer::from_raw(half_w as u32, h as u32, right_data)
            .unwrap_or_else(|| ImageBuffer::from_fn(half_w as u32, h as u32, |_x, _y| Rgb([128u8; 3])));

        debug!("Captured stereo frame: {}x{} + {}x{}", half_w, h, half_w, h);

        Ok(StereoFrame {
            left,
            right,
            timestamp_us: timestamp,
        })
    }

    pub fn save_stereo_pair(&self, frame: &StereoFrame, output_dir: &PathBuf) -> Result<()> {
        std::fs::create_dir_all(output_dir)?;

        let left_path = output_dir.join("left.png");
        let right_path = output_dir.join("right.png");

        frame.left.save(&left_path)?;
        frame.right.save(&right_path)?;

        info!("Stereo pair saved: {:?}, {:?}", left_path, right_path);
        Ok(())
    }
}

pub fn list_cameras() -> Result<Vec<(u32, String)>> {
    platform::list_cameras()
}