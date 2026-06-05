# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

3DME is a Rust-based stereo vision 3D measurement tool using dual synchronized UVC cameras (3840x1080 combined frame, left 1920x1080 + right 1920x1080, 60mm baseline).

## Build Commands

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run
cargo run --bin 3dme --release
```

## Architecture

### Web Server Architecture
The application runs as a web server on port 3030 using `warp` and `tokio`. The frontend is a single HTML/JS page (`static/index.html`) served inline via `include_str!`.

**API Routes**:
- `GET /api/capture` - Capture stereo frame from UVC camera
- `GET /api/image?view=stereo|left|right` - Get current image
- `POST /api/calibrate` - Run chessboard calibration
- `GET /api/parameters` - Load calibration parameters
- `POST /api/point` - Add measurement point (auto-matches right image)

### Camera Configuration
UVC cameras output a single combined 3840x1080 frame (horizontal concatenation of left+right). The `StereoCamera` splits this into two 1920x1080 images. Configuration in `config.toml`:

```toml
[camera]
resolution = [3840, 1080]  # Combined frame
baseline_mm = 60.0         # Camera baseline
focal_length_px = 1200.0  # Initial estimate
principal_point = [1920.0, 540.0]
```

### Core Modules
- `camera/mod.rs` - UVC frame capture via ffmpeg (Windows: dshow, Linux: v4l2)
- `calibration/mod.rs` - Chessboard detection + stereo calibration
- `stereo/mod.rs` - Block-matching disparity computation
- `reconstruct/mod.rs` - Disparity-to-3D point cloud conversion
- `config/mod.rs` - TOML configuration loading

### Calibration Workflow
1. Place calibration pairs in `calibration/` directory: `left_01.png`, `right_01.png`, etc.
2. Chessboard default: 9×6 corners, 30mm squares
3. Calibration runs via web UI or API
4. Parameters saved to `output/stereo_calibration.toml`

### Measurement Workflow
1. Capture stereo pair
2. Load calibration parameters
3. Click on left image to add measurement point
4. System auto-matches right image using SAD block matching
5. Triangulates 3D point using baseline and disparity

### Key Types
- `StereoParameters` - Calibration results (baseline, focal length, principal point)
- `StereoFrame` - Captured left/right image pair
- `MeasurementPoint` - 2D image coordinates + triangulated 3D position

## Platform Notes
- Windows: Uses ffmpeg with DirectShow (`-f dshow`)
- Linux: Uses ffmpeg with V4L2 (`-f v4l2`)
- Requires ffmpeg in PATH for camera capture
