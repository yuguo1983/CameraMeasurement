# 3DME - Stereo Vision Measurement System

Stereo vision 3D measurement tool using dual synchronized UVC cameras.

![Platform](https://img.shields.io/badge/Platform-Windows%20x64-blue)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)
![License](https://img.shields.io/badge/License-MIT-green)

---

## 功能特性

- 实时立体视觉 3D 测量
- 双目摄像头同步采集 (3840x1080)
- 棋盘格自动标定
- 块匹配视差计算
- 点云生成与距离测量
- Web 界面操作，无需安装

---

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10/11 (64-bit) |
| 相机 | UVC 双目摄像头 (3840x1080 输出) |
| 依赖 | ffmpeg |

### 检查 ffmpeg 是否安装

打开命令提示符，输入：

`cmd
ffmpeg -version
`

如果显示版本信息则已安装，否则请前往 [ffmpeg.org](https://ffmpeg.org/download.html) 下载。

---

## 快速开始

### 下载发行版

1. 前往 [Releases](https://github.com/yuguo1983/CameraMeasurement/releases)
2. 下载 3dme-v0.1.0-win-x64.zip
3. 解压到任意目录

### 运行程序

`cmd
双击 3dme.exe
`

程序会自动打开浏览器访问 http://127.0.0.1:3030

---

## 使用流程

### 1. 相机配置

首次使用时，可能需要调整 config.toml 中的相机索引：

`	oml
[camera]
resolution = [3840, 1080]  # 合并帧分辨率
baseline_mm = 60.0         # 双目基线距离 (mm)
focal_length_px = 1200.0   # 焦距 (像素)
`

### 2. 标定

1. 将棋盘格标定板 (9x6 内角点，30mm 方格) 放置在相机前
2. 点击 **Capture** 拍摄一对图像
3. 移动棋盘格到不同位置和角度，拍摄至少 10 对
4. 点击 **Calibrate** 执行标定
5. 标定结果保存到 output/stereo_calibration.toml

### 3. 测量

1. 点击 **Capture** 捕获当前画面
2. 点击左视图中的目标点添加测量点
3. 系统自动在右视图匹配对应点
4. 计算 3D 坐标并显示距离

---

## 测量原理

`
深度 Z = (基线距离 × 焦距) / 视差
X = (左图像素.x - 主点.x) × Z / 焦距
Y = (左图像素.y - 主点.y) × Z / 焦距
`

---

## 目录结构

`
3dme/
├── 3dme.exe           # 主程序
├── config.toml        # 配置文件
└── README.txt         # 本文件

# 程序运行后生成:
├── calibration/       # 标定图像存储
├── output/            # 标定结果输出
│   └── stereo_calibration.toml
└── target/            # 编译缓存 (源代码版本)
`

---

## 配置说明

编辑 config.toml:

| 参数 | 说明 | 默认值 |
|------|------|--------|
| camera.baseline_mm | 双目基线距离 (mm) | 60.0 |
| camera.focal_length_px | 焦距 (像素) | 1200.0 |
| camera.principal_point | 主点坐标 | [1920, 540] |
| stereo.block_size | 匹配块大小 | 9 |
| stereo.max_disparity | 最大视差搜索范围 | 128 |
| econstruct.depth_min_mm | 最小测量距离 (mm) | 100 |
| econstruct.depth_max_mm | 最大测量距离 (mm) | 5000 |

---

## 技术栈

| 组件 | 技术 |
|------|------|
| 后端框架 | Rust + Warp + Tokio |
| 图像处理 | image, imageproc |
| 数学计算 | nalgebra |
| 并行计算 | Rayon |
| 相机驱动 | ffmpeg (dshow) |
| 前端 | 原生 HTML + CSS + JavaScript |

---

## 构建源码

### 环境准备

1. 安装 [Rust](https://rustup.rs/)
2. 安装 [ffmpeg](https://ffmpeg.org/download.html)

### 编译

`cmd
# 开发版本
cargo build

# 发布版本
cargo build --release

# 运行
cargo run --release
`

---

## 常见问题

**Q: 启动后无法打开相机？**
- 确认相机已正确连接 USB
- 确认 ffmpeg 已安装并在 PATH 中
- 尝试修改 config.toml 中的 Camera Index

**Q: 标定失败？**
- 确保 calibration/ 目录至少有 3 对有效图像
- 棋盘格必须完整出现在左右图像中
- 尝试在不同距离和角度拍摄更多图像

**Q: 测量数据不准确？**
- 执行标定获取精确参数
- 确认 aseline_mm 与实际基线一致
- 选择有清晰纹理特征的点

---

## 项目地址

https://github.com/yuguo1983/CameraMeasurement

---

## 许可证

MIT License

---

*最后更新: 2026-06-05*
