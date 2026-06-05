use anyhow::Result;
use image::{ImageBuffer, Rgb, RgbImage};
use log::{debug, error, info};
use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::Arc;
use warp::{Filter, Rejection};

mod calibration;
mod camera;
mod config;
mod reconstruct;
mod stereo;

use calibration::{StereoCalibration, StereoParameters};
use camera::StereoCamera;
use config::Config;
use stereo::compute_disparity;

#[derive(Debug)]
struct AppError(String);
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl warp::reject::Reject for AppError {}

struct AppState {
    config: Config,
    camera: Option<Mutex<StereoCamera>>,
    left_image: Option<RgbImage>,
    right_image: Option<RgbImage>,
    calibration: Option<StereoParameters>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct MeasurementPoint {
    img_x: f64,
    img_y: f64,
    x: f64,
    y: f64,
    z: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("3DME Web Server starting...");

    let config = Config::load(&PathBuf::from("config.toml"))?;
    info!("Configuration loaded");

    let state = Arc::new(RwLock::new(AppState {
        config,
        camera: None,
        left_image: None,
        right_image: None,
        calibration: None,
    }));

    let state_filter = warp::any().map({
        let state = state.clone();
        move || state.clone()
    });

    let index_html = warp::path::end().map(|| {
        warp::reply::html(include_str!("../static/index.html"))
    });

    let tutorial_html = warp::path("tutorial").map(|| {
        warp::reply::html(include_str!("../static/tutorial.html"))
    });

    let capture = warp::path("api")
        .and(warp::path("capture"))
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(|state: Arc<RwLock<AppState>>| async move {
            let mut st = state.write();

            // Reuse cached camera or create one
            if st.camera.is_none() {
                match StereoCamera::new(1, &st.config) {
                    Ok(cam) => st.camera = Some(Mutex::new(cam)),
                    Err(e) => return Err(warp::reject::custom(AppError(e.to_string()))),
                }
            }

            let frame = {
                let mut cam = st.camera.as_ref().unwrap().lock();
                match cam.capture_sync_frame() {
                    Ok(f) => f,
                    Err(e) => return Err(warp::reject::custom(AppError(e.to_string()))),
                }
            };
            st.left_image = Some(frame.left.clone());
            st.right_image = Some(frame.right.clone());
            Ok(warp::reply::json(&serde_json::json!({ "ok": true })))
        });

    let image_api = warp::path("api")
        .and(warp::path("image"))
        .and(warp::get())
        .and(warp::query::<ImageQuery>())
        .and(state_filter.clone())
        .and_then(|query: ImageQuery, state: Arc<RwLock<AppState>>| async move {
            let st = state.read();
            let view = query.view.as_deref().unwrap_or("stereo");

            let (img_opt, _) = match view {
                "stereo" => {
                    if let (Some(left), Some(right)) = (&st.left_image, &st.right_image) {
                        (Some(create_stereo_combined(left, right)), false)
                    } else {
                        (None, false)
                    }
                }
                "left" => (st.left_image.clone(), false),
                "right" => (st.right_image.clone(), false),
                "disparity" => {
                    if let (Some(left), Some(right)) = (&st.left_image, &st.right_image) {
                        let left_dyn = image::DynamicImage::ImageRgb8(left.clone());
                        let right_dyn = image::DynamicImage::ImageRgb8(right.clone());
                        match compute_disparity(&left_dyn, &right_dyn, &st.config.stereo) {
                            Ok(disp) => {
                                // Map grayscale disparity to RGB for display
                                let rgb = image::ImageBuffer::from_fn(disp.width(), disp.height(), |x, y| {
                                    let v = disp.get_pixel(x, y)[0];
                                    image::Rgb([v, v, v])
                                });
                                (Some(rgb), false)
                            }
                            Err(_) => (None, false),
                        }
                    } else {
                        (None, false)
                    }
                }
                _ => (None, false),
            };

            match img_opt {
                Some(img) => {
                    let dyn_img = image::DynamicImage::ImageRgb8(img);
                    let mut buf = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut buf);
                    if dyn_img.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                        Ok(warp::reply::with_header(buf, "Content-Type", "image/png"))
                    } else {
                        Err(warp::reject::not_found())
                    }
                }
                None => Err(warp::reject::not_found()),
            }
        });

    let calibrate = warp::path("api")
        .and(warp::path("calibrate"))
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(|body: CalibrateBody, state: Arc<RwLock<AppState>>| async move {
            // Copy config while holding read lock, then drop it
            let config = { state.read().config.clone() };
            let cal_dir = PathBuf::from(body.directory.as_deref().unwrap_or("calibration"));

            let mut image_pairs = Vec::new();
            if cal_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&cal_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) != Some("png") {
                            continue;
                        }
                        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                        let (is_left, right_name) = if filename.contains("_left") {
                            (true, filename.replace("_left", "_right"))
                        } else if filename.starts_with("left_") || filename.starts_with("left.") {
                            (true, filename.replacen("left", "right", 1))
                        } else {
                            (false, String::new())
                        };

                        if !is_left {
                            continue;
                        }

                        let right_path = path.with_file_name(&right_name);
                        if right_path.exists() {
                            if let (Ok(left_img), Ok(right_img)) = (image::open(&path), image::open(&right_path)) {
                                image_pairs.push((left_img.to_luma8(), right_img.to_luma8()));
                                info!("Calibration pair: {:?} + {:?}", path, right_path);
                            }
                        }
                    }
                }
            }

            if image_pairs.len() < 3 {
                return Err(warp::reject::custom(AppError("Need at least 3 image pairs".to_string())));
            }

            match StereoCalibration::calibrate_board(image_pairs, body.board_cols, body.board_rows, body.square_size_mm, &config) {
                Ok(params) => {
                    std::fs::create_dir_all("output").ok();
                    StereoCalibration::save_parameters(&params, &PathBuf::from("output/stereo_calibration.toml")).ok();
                    let mut st = state.write();
                    st.calibration = Some(params.clone());
                    info!("Calibration complete: baseline={:.1}mm, fx={:.0}px", params.baseline_mm, params.focal_length_px);
                    Ok(warp::reply::json(&params))
                }
                Err(e) => {
                    error!("Calibration failed: {}", e);
                    Err(warp::reject::custom(AppError(e.to_string())))
                }
            }
        });

    let parameters = warp::path("api")
        .and(warp::path("parameters"))
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(|state: Arc<RwLock<AppState>>| async move {
            // Return in-memory calibration if available
            {
                let st = state.read();
                if let Some(params) = &st.calibration {
                    return Ok(warp::reply::json(params));
                }
            }

            // Otherwise try loading from file
            let cal_path = PathBuf::from("output/stereo_calibration.toml");
            if cal_path.exists() {
                match StereoParameters::load(&cal_path) {
                    Ok(params) => {
                        let mut st = state.write();
                        st.calibration = Some(params.clone());
                        Ok(warp::reply::json(&params))
                    }
                    Err(e) => Err(warp::reject::custom(AppError(e.to_string()))),
                }
            } else {
                Err(warp::reject::not_found())
            }
        });

    let point = warp::path("api")
        .and(warp::path("point"))
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(|body: PointBody, state: Arc<RwLock<AppState>>| async move {
            let st = state.read();
            let params = match &st.calibration {
                Some(p) => p,
                None => return Err(warp::reject::not_found()),
            };
            let (left, right) = match (&st.left_image, &st.right_image) {
                (Some(l), Some(r)) => (l, r),
                _ => return Err(warp::reject::not_found()),
            };

            let right_x = match find_corresponding_x(left, right, body.x, body.y) {
                Ok(x) => x,
                Err(_) => return Err(warp::reject::not_found()),
            };

            let point_3d = match params.triangulate((body.x, body.y), (right_x, body.y)) {
                Some(p) => p,
                None => return Err(warp::reject::not_found()),
            };

            let pt = MeasurementPoint {
                img_x: body.x,
                img_y: body.y,
                x: point_3d.x,
                y: point_3d.y,
                z: point_3d.z,
            };
            Ok(warp::reply::json(&pt))
        });

    let save = warp::path("api")
        .and(warp::path("save"))
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(|body: SaveBody, state: Arc<RwLock<AppState>>| async move {
            let st = state.read();
            let dir = std::path::Path::new(&body.directory);
            std::fs::create_dir_all(dir).map_err(|e| {
                warp::reject::custom(AppError(format!("Failed to create directory: {}", e)))
            })?;

            match (&st.left_image, &st.right_image) {
                (Some(left), Some(right)) => {
                    // Auto-number: find the next available index
                    let existing: Vec<_> = std::fs::read_dir(dir)
                        .ok()
                        .into_iter()
                        .flatten()
                        .flatten()
                        .filter_map(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            if name.starts_with("left_") && name.ends_with(".png") {
                                name.trim_start_matches("left_")
                                    .trim_end_matches(".png")
                                    .parse::<u32>().ok()
                            } else {
                                None
                            }
                        })
                        .collect();
                    let next_idx = existing.iter().max().unwrap_or(&0) + 1;

                    let left_name = format!("left_{:02}.png", next_idx);
                    let right_name = format!("right_{:02}.png", next_idx);
                    let left_path = dir.join(&left_name);
                    let right_path = dir.join(&right_name);

                    left.save(&left_path).map_err(|e| {
                        warp::reject::custom(AppError(format!("Failed to save left image: {}", e)))
                    })?;
                    right.save(&right_path).map_err(|e| {
                        warp::reject::custom(AppError(format!("Failed to save right image: {}", e)))
                    })?;
                    Ok(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "index": next_idx,
                        "left": left_path.to_string_lossy(),
                        "right": right_path.to_string_lossy(),
                    })))
                }
                _ => Err(warp::reject::custom(AppError(
                    "No images captured yet. Click Capture first.".to_string(),
                ))),
            }
        });

    let routes = index_html
        .or(tutorial_html)
        .or(capture)
        .or(image_api)
        .or(calibrate)
        .or(parameters)
        .or(point)
        .or(save)
        .with(warp::log("3dme"));

info!("Server starting on http://0.0.0.0:3030 - access from other computers using this machine's LAN IP");

    // Spawn server in background
    let (_addr, server_fut) = warp::serve(routes).bind_ephemeral(([0, 0, 0, 0], 3030));
    let server_handle = tokio::spawn(server_fut);

    // Open browser after a short delay to let the server start
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let url = "http://127.0.0.1:3030";
    if webbrowser::open(url).is_ok() {
        info!("Browser opened to {}", url);
    } else {
        info!("Failed to open browser automatically, please open {} manually", url);
    }

    // Keep server running
    server_handle.await.ok();

    Ok(())
}

fn create_stereo_combined(left: &RgbImage, right: &RgbImage) -> RgbImage {
    let w = left.width();
    let h = left.height();
    let mut combined = ImageBuffer::new(w * 2, h);

    for y in 0..h {
        for x in 0..w {
            combined.put_pixel(x, y, *left.get_pixel(x, y));
            combined.put_pixel(w + x, y, *right.get_pixel(x, y));
        }
    }
    combined
}

fn find_corresponding_x(left: &RgbImage, right: &RgbImage, left_x: f64, left_y: f64) -> Result<f64> {
    let search_window = 80.0;
    let right_start = (left_x - search_window).max(1.0);
    let right_end = (left_x + search_window).min(right.width() as f64 - 1.0);
    let block_half = 2;

    let mut best_x = right_end;
    let mut best_sad = f64::MAX;

    for rx in right_start as i32..=right_end as i32 {
        let mut sad = 0.0;
        let mut count = 0;

        for dy in -block_half..=block_half {
            for dx in -block_half..=block_half {
                let lx = (left_x as i32 + dx).clamp(0, left.width() as i32 - 1) as u32;
                let ly = (left_y as i32 + dy).clamp(0, left.height() as i32 - 1) as u32;
                let rx_c = (rx + dx).clamp(0, right.width() as i32 - 1) as u32;
                let lp = left.get_pixel(lx, ly);
                let rp = right.get_pixel(rx_c, ly);
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
            best_x = rx as f64;
        }
    }

    Ok(best_x)
}

#[derive(Debug, serde::Deserialize)]
struct ImageQuery {
    view: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalibrateBody {
    board_cols: u32,
    board_rows: u32,
    square_size_mm: f64,
    directory: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PointBody {
    x: f64,
    y: f64,
}

#[derive(Debug, serde::Deserialize)]
struct SaveBody {
    directory: String,
}