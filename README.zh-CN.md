<p align="center">
  <img src="misc/images/icon.png" width="128" height="128" alt="Stemcraft 图标">
</p>

<h1 align="center">Stemcraft</h1>

<p align="center">
  <a href="README.md">English</a> · <b>简体中文</b> · <a href="README.zh-TW.md">繁體中文</a>
</p>

把一首歌分离成六条音轨：鼓、贝斯、人声、钢琴、其他、吉他，再挑出想要的几条，导出成新的混音。
灵感来自 [Haig012/guitar-extractor](https://github.com/Haig012/guitar-extractor)，用 Rust 重写。

项目包含命令行工具和 macOS 桌面应用。桌面应用可以分离整首歌或其中一段，边听边对六条音轨做静音、独奏和音量调节，
然后导出混音（也可以同时导出各条单轨和和弦谱），格式可选 WAV、FLAC、MP3、M4A（AAC）或 OGG。

分离使用 Demucs `htdemucs_6s` 模型，通过 [demucs-rs](https://github.com/nikhilunni/demucs-rs) /
[Burn](https://burn.dev) 在 GPU（Metal）上运行，不需要 Python、PyTorch 或 ffmpeg。
GPUI（[gpui-kit](https://github.com/longbridge/gpui-kit)）桌面应用与命令行工具共用同一个核心库。

![Stemcraft 混音台（深色主题）](misc/images/screenshot-mixer.png)

## 目录结构

| 路径 | 内容 |
|---|---|
| `crates/core` | 核心库：解码（Symphonia）、分离、吉他 / 伴奏混音、和弦识别、模型权重缓存 |
| `crates/cli` | `stemcraft` 命令行工具 |
| `crates/app` | `stemcraft-app` 桌面应用（GPUI） |
| `misc/python_src` | 早期的 Python 原型（demucs-mlx），留作参考 |

## 构建

需要 Rust 1.98.1（已在 `rust-toolchain.toml` 中固定，rustup 会自动安装）。

```bash
cargo build --release
```

## 桌面应用

```bash
cargo run --release -p stemcraft-app      # 从源码运行
script/bundle-macos.sh                    # → dist/Stemcraft.app 和 dist/Stemcraft-<version>.dmg
```

打包好的应用把模型权重放在 `Contents/Resources/` 里，离线即可使用，不需要另外安装任何东西。
应用只做了 ad-hoc 签名，所以在其他 Mac 上第一次打开会被拦截：打开“系统设置 → 隐私与安全性”，点“仍要打开”即可
（macOS 15 之前的版本也可以右键点应用选“打开”）。要分发给别人，需要 Developer ID 签名和公证。

设置（Stemcraft → 设置…，⌘,）包括：语言（English、简体中文、繁體中文，默认跟随系统）、外观（跟随系统 / 浅色 / 深色）、
导出位置和导出默认值、音频输出设备、GPU 优化缓存，以及第三方许可。设置保存在
`~/Library/Application Support/Stemcraft/settings.json`。

## 命令行用法

```bash
stemcraft song.mp3                 # → output/song/song_guitar.wav、song_no_guitar.wav
stemcraft song.mp3 --chords        # 另外输出 song_chords.lrc / .txt
stemcraft song.mp3 -r 1:30-3:00    # 只处理歌曲的一段
stemcraft song.mp3 --all-stems     # 另外把六条原始音轨输出到 output/song/stems/
```

（也可以用 `cargo run --release -- song.mp3 …`。）支持的输入格式：WAV、AIFF、FLAC、MP3、OGG Vorbis、
M4A（AAC/ALAC），单声道或立体声。输出为 32 位浮点 WAV，采样率与输入相同，所以叠加出的伴奏轨不会削波。

第一次运行会从 Hugging Face 下载模型权重（约 55 MB）到 `~/Library/Caches/demucs-rs/`，
并编译、自动调优 GPU 内核（缓存在 `~/Library/Application Support/autotune/`）；之后的运行会立即开始。

## 测试

```bash
cargo test
```

## 许可证

Stemcraft 以 [MIT 许可证](LICENSE) 发布。打包进应用的第三方组件（LAME、libvorbis、gpui 等）
沿用各自的许可证，可以在应用的“设置 → 关于”中查看。
