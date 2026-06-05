# Git 操作指南

本项目使用 Git 进行版本控制，托管在 GitHub 上。

## 仓库信息

- **仓库地址**: https://github.com/yuguo1983/CameraMeasurement
- **本地路径**: F:\3dme1\3dme

---

## 日常开发流程

### 1. 拉取最新代码

\\\ash
git pull origin main
\\\

### 2. 查看当前状态

\\\ash
git status
\\\

### 3. 添加文件到暂存区

\\\ash
# 添加单个文件
git add filename

# 添加所有更改（包括新文件、修改、删除）
git add -A

# 添加所有修改的文件（不包括新文件）
git add -u

# 添加特定目录
git add src/
git add static/
\\\

### 4. 提交更改

\\\ash
git commit -m \"提交信息\"
\\\

**提交信息规范**:
- feat: 新功能
- fix: 修复 bug
- docs: 文档更新
- style: 代码格式（不影响功能）
- refactor: 重构
- perf: 性能优化
- test: 测试相关
- chore: 构建/工具相关

**示例**:
\\\ash
git commit -m \"feat: 添加点云导出功能\"
git commit -m \"fix: 修复视差计算边界问题\"
\\\

### 5. 推送到远程仓库

\\\ash
# 推送到 main 分支
git push origin main

# 如果有新的远程分支
git push -u origin branch-name
\\\

---

## 分支管理

### 查看分支

\\\ash
# 查看本地分支
git branch

# 查看所有分支（包括远程）
git branch -a
\\\

### 创建新分支

\\\ash
git branch feature-name
git checkout feature-name
# 或
git checkout -b feature-name
\\\

### 切换分支

\\\ash
git checkout main
git checkout feature-name
\\\

### 合并分支

\\\ash
# 1. 切换到目标分支（通常是 main）
git checkout main

# 2. 合并特性分支
git merge feature-name

# 3. 删除已合并的分支
git branch -d feature-name
\\\

### 删除分支

\\\ash
# 删除本地分支
git branch -d branch-name

# 强制删除（未合并的分支）
git branch -D branch-name

# 删除远程分支
git push origin --delete branch-name
\\\

---

## GitHub Release 发布流程

### 1. 确保代码已提交

\\\ash
git add -A
git commit -m \"Release v0.x.x\"
\\\

### 2. 创建并推送 Tag

\\\ash
# 创建 Tag
git tag v0.1.0

# 推送 Tag 到远程
git push origin v0.1.0
\\\

### 3. 创建 Release（通过 GitHub API）

\\\ash
# 使用 curl 或 PowerShell 调用 GitHub API
curl -X POST \\
  -H \"Authorization: token YOUR_TOKEN\" \\
  -H \"Content-Type: application/json\" \\
  -d '{\"tag_name\":\"v0.1.0\",\"name\":\"v0.1.0\",\"body\":\"Release description\"}' \\
  https://api.github.com/repos/yuguo1983/CameraMeasurement/releases
\\\

### 4. 上传附件到 Release

\\\ash
# 上传 zip 文件
curl -X POST \\
  -H \"Authorization: token YOUR_TOKEN\" \\
  -H \"Content-Type: application/zip\" \\
  --data-binary \"@release.zip\" \\
  \"https://uploads.github.com/repos/yuguo1983/CameraMeasurement/releases/RELEASE_ID/assets?name=release.zip\"
\\\

### 5. 通过网页手动发布

1. 打开 https://github.com/yuguo1983/CameraMeasurement/releases
2. 点击 **Draft a new release**
3. 选择 tag，填写标题和描述
4. 上传附件（exe、zip 等）
5. 点击 **Publish release**

---

## 编译发布版本

### 编译 Release 版本

\\\ash
# 开发编译
cargo build

# Release 编译（优化）
cargo build --release
\\\

### 创建发行版目录

\\\ash
# 创建目录
mkdir release/3dme-v0.1.0-win-x64

# 复制文件
copy target/release/3dme.exe release/3dme-v0.1.0-win-x64/
copy config.toml release/3dme-v0.1.0-win-x64/
\\\

### 打包发行版

\\\ash
# Windows PowerShell
Compress-Archive -Path \"release/3dme-v0.1.0-win-x64\" -DestinationPath \"release/3dme-v0.1.0-win-x64.zip\"
\\\

---

## Git 配置

### 查看配置

\\\ash
git config --list
git config user.name
git config user.email
\\\

### 设置用户信息

\\\ash
git config user.email \"your@email.com\"
git config user.name \"Your Name\"
\\\

### 设置凭证存储

\\\ash
# 存储在本地（明文，不推荐）
git config credential.helper store

# 使用 Windows 凭证管理器
git config credential.helper manager
\\\

---

## 常用 Git 别名

\\\ash
# 在 .gitconfig 中添加
[alias]
    st = status
    co = checkout
    br = branch
    ci = commit
    lg = log --oneline --graph --decorate
    unstage = reset HEAD --
    last = log -1 HEAD
\\\

---

## 常见问题

### Q: 如何撤销未提交的修改？

\\\ash
# 撤销单个文件
git checkout -- filename
git restore filename

# 撤销所有未提交的修改
git checkout -- .
git restore .
\\\

### Q: 如何撤销已暂存的文件？

\\\ash
git reset HEAD filename
\\\

### Q: 如何修改最近的提交？

\\\ash
# 修改提交信息
git commit --amend -m \"New message\"

# 添加遗漏的文件到上次提交
git add forgotten-file
git commit --amend --no-edit
\\\

### Q: 如何查看提交历史？

\\\ash
# 简洁日志
git log --oneline

# 图形化日志
git log --graph --oneline --all

# 特定文件的历史
git log filename
\\\

### Q: 如何处理合并冲突？

1. 打开冲突文件，查找 \<<<<<<<\, \=======\, \>>>>>>>\ 标记
2. 手动编辑解决冲突
3. 保存文件
4. 执行:
   \\\ash
   git add filename
   git commit
   \\\

---

## 文件忽略 (.gitignore)

本项目已配置以下忽略规则:

\\\
/target    # Rust 编译输出
\\\

如果需要忽略其他文件（如标定图片），创建 .gitignore 并添加:

\\\
/calibration/
/output/
*.png
*.toml
\\\

---

**最后更新**: 2026-06-05
