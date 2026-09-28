//! UI copy in English and Simplified Chinese; switchable at runtime.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::settings::Language;

pub struct Strings {
    pub drop_hint: &'static str,
    pub choose_file: &'static str,
    pub supported_formats: &'static str,
    pub reading: &'static str,
    pub unsupported_file: &'static str,
    pub open_failed: &'static str,
    /// Joins a notification's headline to its reason, e.g. `{open_failed}{reason_sep}{e}`.
    pub reason_sep: &'static str,
    pub worker_stopped: &'static str,
    pub play: &'static str,
    pub pause: &'static str,
    pub change_file: &'static str,
    pub trim_hint: &'static str,
    pub start_label: &'static str,
    pub end_label: &'static str,
    /// `{len}`
    pub selected_length: &'static str,
    pub bad_time: &'static str,
    pub too_short: &'static str,
    pub start_separation: &'static str,
    pub processing_title: &'static str,
    pub step_load_model: &'static str,
    pub step_warmup: &'static str,
    pub step_separate: &'static str,
    pub step_chords: &'static str,
    pub failed_title: &'static str,
    pub retry: &'static str,
    pub open_new: &'static str,
    pub confirm_discard_title: &'static str,
    pub confirm_discard_body: &'static str,
    pub confirm_open: &'static str,
    pub cancel: &'static str,
    pub chords_label: &'static str,
    pub no_audio_device: &'static str,
    /// Display names in `worker::STEM_IDS` order.
    pub stems: [&'static str; 6],
    pub export: &'static str,
    pub export_title: &'static str,
    pub format_label: &'static str,
    pub mix_label: &'static str,
    pub also_stems: &'static str,
    pub chord_chart: &'static str,
    pub save_to: &'static str,
    pub change_folder: &'static str,
    pub export_button: &'static str,
    /// `{done}`, `{total}`
    pub exporting: &'static str,
    pub export_done: &'static str,
    pub export_failed: &'static str,
    pub reveal: &'static str,
    pub quit: &'static str,
    pub back_to_start: &'static str,
    pub open_short: &'static str,
    pub settings_tooltip: &'static str,
    pub settings_menu: &'static str,
    /// `{rate}`
    pub stereo_rate: &'static str,
    /// `{start}`, `{end}`
    pub range: &'static str,
    pub tracks_label: &'static str,
    pub system_default_device: &'static str,
    pub device_fallback: &'static str,
    /// `{name}`
    pub unavailable: &'static str,
    pub export_busy: &'static str,
    pub export_contents: &'static str,
    pub export_dir_fallback: &'static str,
    pub settings_save_failed: &'static str,
    pub settings_title: &'static str,
    pub page_general: &'static str,
    pub page_export: &'static str,
    pub page_audio: &'static str,
    pub page_storage: &'static str,
    pub page_about: &'static str,
    pub language_label: &'static str,
    pub follow_system: &'static str,
    pub appearance_label: &'static str,
    pub light: &'static str,
    pub dark: &'static str,
    pub export_location: &'static str,
    pub location_same: &'static str,
    pub location_folder: &'static str,
    pub location_desc: &'static str,
    pub folder_label: &'static str,
    pub choose: &'static str,
    pub default_format: &'static str,
    pub defaults_desc: &'static str,
    pub default_stems: &'static str,
    pub default_chords: &'static str,
    pub output_device: &'static str,
    pub output_device_desc: &'static str,
    pub gpu_cache: &'static str,
    pub gpu_cache_desc: &'static str,
    pub clear: &'static str,
    pub clear_confirm_title: &'static str,
    pub clear_confirm_body: &'static str,
    pub clear_busy: &'static str,
    pub cache_cleared: &'static str,
    pub clear_failed: &'static str,
    pub model_label: &'static str,
    pub model_bundled: &'static str,
    pub model_cached: &'static str,
    pub tagline: &'static str,
    pub version_label: &'static str,
    pub model_source: &'static str,
    pub model_source_value: &'static str,
    pub licenses: &'static str,
}

pub static EN: Strings = Strings {
    drop_hint: "Drop a song here",
    choose_file: "Choose file…",
    supported_formats: "WAV, AIFF, FLAC, MP3, OGG, or M4A",
    reading: "Reading the song…",
    unsupported_file: "That file type isn't supported. Choose a WAV, AIFF, FLAC, MP3, OGG, or M4A file.",
    open_failed: "Couldn't read this file",
    reason_sep: ": ",
    worker_stopped: "The background task stopped unexpectedly",
    play: "Play",
    pause: "Pause",
    change_file: "Choose another song",
    trim_hint: "Drag across the waveform to pick the part to split. The whole song is selected by default.",
    start_label: "Start",
    end_label: "End",
    selected_length: "Selected: {len}",
    bad_time: "Use minutes:seconds, like 1:30",
    too_short: "Pick at least 1 second",
    start_separation: "Split into stems",
    processing_title: "Splitting",
    step_load_model: "Loading the model",
    step_warmup: "Optimizing for your GPU (first run only, takes a minute or two)",
    step_separate: "Separating stems",
    step_chords: "Detecting chords",
    failed_title: "Couldn't split this song",
    retry: "Try again",
    open_new: "Open another song",
    confirm_discard_title: "Open another song?",
    confirm_discard_body: "The current stems will be discarded. Export them first if you want to keep them.",
    confirm_open: "Open",
    cancel: "Cancel",
    chords_label: "Chords",
    no_audio_device: "Can't play audio",
    stems: ["Drums", "Bass", "Vocals", "Piano", "Guitar", "Other"],
    export: "Export…",
    export_title: "Export",
    format_label: "Format",
    mix_label: "Mix, as you hear it",
    also_stems: "Also export individual stems",
    chord_chart: "Chord chart (.lrc and .txt)",
    save_to: "Save to",
    change_folder: "Change…",
    export_button: "Export",
    exporting: "Exporting {done}/{total}",
    export_done: "Export finished",
    export_failed: "Export failed",
    reveal: "Show in Finder",
    quit: "Quit Stemcraft",
    back_to_start: "Back to start",
    open_short: "Open",
    settings_tooltip: "Settings (⌘,)",
    settings_menu: "Settings…",
    stereo_rate: "{rate} kHz · Stereo",
    range: "Selection {start} – {end}",
    tracks_label: "Tracks",
    system_default_device: "System default",
    device_fallback: "The chosen output device isn't available, so the system default is used.",
    unavailable: "{name} (unavailable)",
    export_busy: "An export is already running.",
    export_contents: "Contents",
    export_dir_fallback: "The export folder no longer exists, so the song's folder is used.",
    settings_save_failed: "Couldn't save settings",
    settings_title: "Settings",
    page_general: "General",
    page_export: "Export",
    page_audio: "Audio",
    page_storage: "Storage",
    page_about: "About",
    language_label: "Language",
    follow_system: "Follow system",
    appearance_label: "Appearance",
    light: "Light",
    dark: "Dark",
    export_location: "Export location",
    location_same: "Same folder as the song",
    location_folder: "Custom folder",
    location_desc: "You can still pick another folder when exporting.",
    folder_label: "Folder",
    choose: "Choose…",
    default_format: "Default format",
    defaults_desc: "Pre-selected in the export dialog.",
    default_stems: "Also export individual stems",
    default_chords: "Export the chord chart",
    output_device: "Output device",
    output_device_desc: "Used for listening. Stemcraft never changes the device's sample rate.",
    gpu_cache: "GPU optimization cache",
    gpu_cache_desc: "After clearing, the next split re-optimizes for your GPU. The folder is shared with other apps built on the same GPU library.",
    clear: "Clear",
    clear_confirm_title: "Clear the GPU optimization cache?",
    clear_confirm_body: "The next split takes a minute or two longer while it re-optimizes.",
    clear_busy: "Can't clear the cache while a song is being split.",
    cache_cleared: "Cache cleared",
    clear_failed: "Couldn't clear the cache",
    model_label: "Model",
    model_bundled: "htdemucs_6s, built into the app",
    model_cached: "htdemucs_6s, in the download cache",
    tagline: "Split a song into six stems, mix them, and export.",
    version_label: "Version",
    model_source: "Model source",
    model_source_value: "Demucs by Meta Research, MIT license. No separate license is stated for the model weights.",
    licenses: "Third-party licenses",
};

pub static ZH: Strings = Strings {
    drop_hint: "把歌曲拖到这里",
    choose_file: "选择文件…",
    supported_formats: "支持 WAV、AIFF、FLAC、MP3、OGG、M4A",
    reading: "正在读取…",
    unsupported_file: "不支持这种文件。选择 WAV、AIFF、FLAC、MP3、OGG 或 M4A 文件。",
    open_failed: "无法读取这个文件",
    reason_sep: "：",
    worker_stopped: "后台任务意外中止",
    play: "播放",
    pause: "暂停",
    change_file: "换一个文件",
    trim_hint: "在波形上拖动，选择要分离的片段；默认是整首。",
    start_label: "开始",
    end_label: "结束",
    selected_length: "已选 {len}",
    bad_time: "时间格式应为 分:秒，例如 1:30",
    too_short: "选段至少要 1 秒",
    start_separation: "开始分离",
    processing_title: "正在分离",
    step_load_model: "加载模型",
    step_warmup: "优化 GPU（仅首次，需要一两分钟）",
    step_separate: "分离音轨",
    step_chords: "识别和弦",
    failed_title: "分离失败",
    retry: "重试",
    open_new: "打开新文件",
    confirm_discard_title: "打开新文件？",
    confirm_discard_body: "当前的分离结果会被丢弃。如需保留，先导出。",
    confirm_open: "打开",
    cancel: "取消",
    chords_label: "和弦",
    no_audio_device: "无法播放",
    stems: ["鼓", "贝斯", "人声", "钢琴", "吉他", "其他"],
    export: "导出…",
    export_title: "导出",
    format_label: "格式",
    mix_label: "混音（与试听一致）",
    also_stems: "同时导出单轨",
    chord_chart: "和弦谱（.lrc 和 .txt）",
    save_to: "保存到",
    change_folder: "更改…",
    export_button: "导出",
    exporting: "正在导出 {done}/{total}",
    export_done: "导出完成",
    export_failed: "导出失败",
    reveal: "在访达中显示",
    quit: "退出 Stemcraft",
    back_to_start: "回到开头",
    open_short: "打开",
    settings_tooltip: "设置（⌘,）",
    settings_menu: "设置…",
    stereo_rate: "{rate} kHz · 立体声",
    range: "选段 {start} – {end}",
    tracks_label: "音轨",
    system_default_device: "系统默认",
    device_fallback: "所选的输出设备不可用，已改用系统默认设备。",
    unavailable: "{name}（不可用）",
    export_busy: "已有导出正在进行。",
    export_contents: "内容",
    export_dir_fallback: "指定的导出文件夹已不存在，改用原曲所在的文件夹。",
    settings_save_failed: "无法保存设置",
    settings_title: "设置",
    page_general: "通用",
    page_export: "导出",
    page_audio: "音频",
    page_storage: "存储",
    page_about: "关于",
    language_label: "语言",
    follow_system: "跟随系统",
    appearance_label: "外观",
    light: "浅色",
    dark: "深色",
    export_location: "导出位置",
    location_same: "与原曲相同的文件夹",
    location_folder: "指定文件夹",
    location_desc: "导出时仍可临时选择其他文件夹。",
    folder_label: "文件夹",
    choose: "选择…",
    default_format: "默认格式",
    defaults_desc: "导出对话框会预先选好这些。",
    default_stems: "同时导出单轨",
    default_chords: "导出和弦谱",
    output_device: "输出设备",
    output_device_desc: "用于试听。Stemcraft 不会改动设备的采样率。",
    gpu_cache: "GPU 优化缓存",
    gpu_cache_desc: "清除后，下次分离会重新优化 GPU。这个文件夹与使用同一 GPU 库的其他应用共用。",
    clear: "清除",
    clear_confirm_title: "清除 GPU 优化缓存？",
    clear_confirm_body: "下次分离会多花一两分钟重新优化。",
    clear_busy: "正在分离歌曲，暂时不能清除缓存。",
    cache_cleared: "缓存已清除",
    clear_failed: "无法清除缓存",
    model_label: "模型",
    model_bundled: "htdemucs_6s，内置于应用",
    model_cached: "htdemucs_6s，位于下载缓存",
    tagline: "把歌曲分离成六条音轨，混音后导出。",
    version_label: "版本",
    model_source: "模型来源",
    model_source_value: "Demucs（Meta Research），MIT 许可。上游未单独声明模型权重的许可。",
    licenses: "第三方许可",
};

/// 0 = English, 1 = Chinese.
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn is_chinese(locale: Option<&str>) -> bool {
    locale.is_some_and(|l| l.to_ascii_lowercase().starts_with("zh"))
}

/// The strings a language setting resolves to; `System` follows the OS's
/// first preferred language.
pub fn resolve(language: Language, system_locale: Option<&str>) -> &'static Strings {
    match language {
        Language::ZhHans => &ZH,
        Language::English => &EN,
        Language::System if is_chinese(system_locale) => &ZH,
        Language::System => &EN,
    }
}

/// Switch the UI language. Returns gpui-kit's locale code, so its built-in
/// strings can follow.
pub fn set_language(language: Language, system_locale: Option<&str>) -> &'static str {
    let chinese = std::ptr::eq(resolve(language, system_locale), &ZH);
    CURRENT.store(u8::from(chinese), Ordering::Relaxed);
    if chinese { "zh-CN" } else { "en" }
}

pub fn t() -> &'static Strings {
    if CURRENT.load(Ordering::Relaxed) == 1 { &ZH } else { &EN }
}

/// Replace `{key}` placeholders.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    args.iter()
        .fold(template.to_string(), |text, (key, value)| text.replace(&format!("{{{key}}}"), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_only_for_zh_locales() {
        assert!(is_chinese(Some("zh-CN")));
        assert!(is_chinese(Some("zh-Hans-JP")));
        assert!(is_chinese(Some("ZH_tw")));
        assert!(!is_chinese(Some("en-JP")));
        assert!(!is_chinese(None));
    }

    #[test]
    fn fill_replaces_placeholders() {
        assert_eq!(fill("{done}/{total}", &[("done", "3"), ("total", "7")]), "3/7");
    }

    #[test]
    fn falls_back_to_english_before_init() {
        assert_eq!(t().stems.len(), 6);
    }

    #[test]
    fn chinese_reason_separator_is_full_width() {
        assert_eq!(ZH.reason_sep, "：");
        assert_eq!(EN.reason_sep, ": ");
    }

    #[test]
    fn resolves_languages() {
        use crate::settings::Language;
        assert!(std::ptr::eq(resolve(Language::ZhHans, Some("en-US")), &ZH));
        assert!(std::ptr::eq(resolve(Language::English, Some("zh-CN")), &EN));
        assert!(std::ptr::eq(resolve(Language::System, Some("zh-Hans-JP")), &ZH));
        assert!(std::ptr::eq(resolve(Language::System, Some("en-JP")), &EN));
        assert!(std::ptr::eq(resolve(Language::System, None), &EN));
    }

    #[test]
    fn set_language_switches_strings_and_reports_locale() {
        use crate::settings::Language;
        assert_eq!(set_language(Language::ZhHans, None), "zh-CN");
        assert_eq!(t().play, "播放");
        assert_eq!(set_language(Language::English, None), "en");
        assert_eq!(t().play, "Play");
    }
}
