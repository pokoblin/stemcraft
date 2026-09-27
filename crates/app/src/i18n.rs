//! UI copy in English and Simplified Chinese, picked from the system's first
//! preferred language.

use std::sync::OnceLock;

pub struct Strings {
    pub drop_hint: &'static str,
    pub choose_file: &'static str,
    pub supported_formats: &'static str,
    pub reading: &'static str,
    pub unsupported_file: &'static str,
    pub open_failed: &'static str,
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
}

pub static EN: Strings = Strings {
    drop_hint: "Drop a song here",
    choose_file: "Choose file…",
    supported_formats: "WAV, AIFF, FLAC, MP3, OGG, or M4A",
    reading: "Reading the song…",
    unsupported_file: "That file type isn't supported. Choose a WAV, AIFF, FLAC, MP3, OGG, or M4A file.",
    open_failed: "Couldn't read this file",
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
};

pub static ZH: Strings = Strings {
    drop_hint: "把歌曲拖到这里",
    choose_file: "选择文件…",
    supported_formats: "支持 WAV、AIFF、FLAC、MP3、OGG、M4A",
    reading: "正在读取…",
    unsupported_file: "不支持这种文件，请选择 WAV、AIFF、FLAC、MP3、OGG 或 M4A 文件。",
    open_failed: "无法读取这个文件",
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
    confirm_discard_body: "当前的分离结果会被丢弃。如需保留，请先导出。",
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
};

static CURRENT: OnceLock<&'static Strings> = OnceLock::new();

pub fn is_chinese(locale: Option<&str>) -> bool {
    locale.is_some_and(|l| l.to_ascii_lowercase().starts_with("zh"))
}

/// Pick the language once at startup (`sys_locale::get_locale()`).
pub fn init(locale: Option<&str>) {
    let _ = CURRENT.set(if is_chinese(locale) { &ZH } else { &EN });
}

pub fn t() -> &'static Strings {
    CURRENT.get().copied().unwrap_or(&EN)
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
}
