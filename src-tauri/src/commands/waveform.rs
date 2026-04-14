use tauri::State;

use crate::commands::editor::EditorStore;

const EXPORT_SEAM_FADE_US: i64 = 8_000;

/// A keep-segment: contiguous non-deleted region of the source media.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct KeepSegment {
    pub start_us: i64,
    pub end_us: i64,
}

/// Generate waveform peaks from a WAV audio file.
///
/// Returns `peak_count` normalized peak values (0.0–1.0) suitable for rendering
/// a bar-chart waveform. Falls back gracefully if the file cannot be decoded.
#[tauri::command]
#[specta::specta]
pub fn generate_waveform_peaks(
    path: String,
    peak_count: Option<usize>,
) -> Result<Vec<f32>, String> {
    let count = peak_count.unwrap_or(300);
    if count == 0 {
        return Err("peak_count must be > 0".to_string());
    }

    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // Read WAV samples via hound
    let samples = crate::audio_toolkit::read_wav_samples(file_path)
        .map_err(|e| format!("Failed to read audio: {}", e))?;

    if samples.is_empty() {
        return Ok(vec![0.0; count]);
    }

    // Downsample into peaks
    let block_size = samples.len() / count;
    if block_size == 0 {
        // Fewer samples than peaks — pad with zeros
        let mut peaks: Vec<f32> = samples.iter().map(|s| s.abs()).collect();
        peaks.resize(count, 0.0);
        return Ok(normalize_peaks(peaks));
    }

    let mut peaks = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * block_size;
        let end = if i == count - 1 {
            samples.len()
        } else {
            (i + 1) * block_size
        };
        let max = samples[start..end]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        peaks.push(max);
    }

    Ok(normalize_peaks(peaks))
}

fn normalize_peaks(mut peaks: Vec<f32>) -> Vec<f32> {
    let global_max = peaks.iter().copied().fold(0.01_f32, f32::max);
    for p in &mut peaks {
        *p /= global_max;
    }
    peaks
}

fn seam_fade_duration_seconds(start_us: i64, end_us: i64) -> Option<f64> {
    let duration_us = (end_us - start_us).max(0);
    let fade_us = EXPORT_SEAM_FADE_US.min(duration_us / 2);
    (fade_us > 0).then_some(fade_us as f64 / 1_000_000.0)
}

fn build_audio_segment_filter(
    index: usize,
    segment_count: usize,
    start_us: i64,
    end_us: i64,
) -> String {
    let start_s = start_us as f64 / 1_000_000.0;
    let end_s = end_us as f64 / 1_000_000.0;
    let duration_s = ((end_us - start_us).max(0)) as f64 / 1_000_000.0;

    let mut filter = format!("[0:a]atrim=start={start_s:.6}:end={end_s:.6},asetpts=PTS-STARTPTS");

    if let Some(fade_s) = seam_fade_duration_seconds(start_us, end_us) {
        if index > 0 {
            filter.push_str(&format!(",afade=t=in:st=0:d={fade_s:.6}"));
        }
        if index + 1 < segment_count {
            let fade_out_start_s = (duration_s - fade_s).max(0.0);
            filter.push_str(&format!(
                ",afade=t=out:st={fade_out_start_s:.6}:d={fade_s:.6}"
            ));
        }
    }

    filter.push_str(&format!("[a{index}]"));
    filter
}

/// Get the keep-segments (non-deleted contiguous regions) from the editor.
#[tauri::command]
#[specta::specta]
pub fn get_keep_segments(store: State<EditorStore>) -> Result<Vec<KeepSegment>, String> {
    let state = store.0.lock().unwrap();
    let segments = state
        .get_keep_segments()
        .into_iter()
        .map(|(start_us, end_us)| KeepSegment { start_us, end_us })
        .collect();
    Ok(segments)
}

/// Generate an FFmpeg concat filter script from keep-segments.
///
/// This produces a filter_complex command that can be run with FFmpeg CLI
/// to trim and concatenate the kept portions of the source media.
///
/// Usage: `ffmpeg -i <input> -filter_complex "<output>" -map "[outv]" -map "[outa]" <output_file>`
#[tauri::command]
#[specta::specta]
pub fn generate_ffmpeg_edit_script(
    store: State<EditorStore>,
    input_path: String,
) -> Result<String, String> {
    let state = store.0.lock().unwrap();
    let segments = state.get_keep_segments();

    if segments.is_empty() {
        return Err("No segments to export (all words deleted)".to_string());
    }

    // Build an FFmpeg command line using -ss/-to trim + concat demuxer approach
    let mut lines = Vec::new();
    lines.push(format!("# FFmpeg edit script for: {}", input_path));
    lines.push(format!("# {} segment(s) to keep\n", segments.len()));

    if segments.len() == 1 {
        // Single segment — simple trim
        let (start, end) = segments[0];
        let start_s = start as f64 / 1_000_000.0;
        let end_s = end as f64 / 1_000_000.0;
        lines.push(format!(
            "ffmpeg -i \"{}\" -ss {:.6} -to {:.6} -c copy \"output.mp4\"",
            input_path, start_s, end_s
        ));
    } else {
        // Multiple segments — filter_complex with trim + concat
        let mut filter_parts = Vec::new();
        let n = segments.len();

        for (i, (start, end)) in segments.iter().enumerate() {
            let start_s = *start as f64 / 1_000_000.0;
            let end_s = *end as f64 / 1_000_000.0;
            filter_parts.push(format!(
                "[0:v]trim=start={:.6}:end={:.6},setpts=PTS-STARTPTS[v{i}]; \
                 [0:a]atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS[a{i}]",
                start_s, end_s, start_s, end_s
            ));
        }

        let v_inputs: String = (0..n).map(|i| format!("[v{i}]")).collect();
        let a_inputs: String = (0..n).map(|i| format!("[a{i}]")).collect();
        filter_parts.push(format!(
            "{v_inputs}concat=n={n}:v=1:a=0[outv]; {a_inputs}concat=n={n}:v=0:a=1[outa]"
        ));

        let filter = filter_parts.join("; ");
        lines.push(format!(
            "ffmpeg -i \"{}\" -filter_complex \"{}\" -map \"[outv]\" -map \"[outa]\" \"output.mp4\"",
            input_path, filter
        ));
    }

    Ok(lines.join("\n"))
}

/// Map an edit-timeline position back to the source-media position.
///
/// When words are deleted, the edited timeline is shorter than the source.
/// This maps a position on the edit timeline to the corresponding source time.
#[tauri::command]
#[specta::specta]
pub fn map_edit_to_source_time(
    store: State<EditorStore>,
    edit_time_us: i64,
) -> Result<i64, String> {
    let state = store.0.lock().unwrap();
    Ok(state.map_edit_time_to_source_time(edit_time_us))
}

/// Export the edited media by running FFmpeg with trim/atrim filters.
///
/// Uses the keep-segments from the editor to produce an output file
/// with deleted segments removed. Supports both audio-only and video+audio.
#[tauri::command]
#[specta::specta]
pub async fn export_edited_media(
    store: State<'_, EditorStore>,
    input_path: String,
    output_path: String,
) -> Result<String, String> {
    let segments = {
        let state = store.0.lock().unwrap();
        state.get_keep_segments()
    };

    if segments.is_empty() {
        return Err("No segments to export (all words deleted)".to_string());
    }

    let input = std::path::Path::new(&input_path);
    if !input.exists() {
        return Err(format!("Input file not found: {}", input_path));
    }

    // Detect if input has video by checking extension
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let has_video = matches!(ext.as_str(), "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv");

    let mut args: Vec<String> = vec!["-y".to_string(), "-i".to_string(), input_path.clone()];

    if segments.len() == 1 {
        // Single segment — simple trim with re-encode for sample-accurate cuts
        let (start, end) = segments[0];
        let start_s = start as f64 / 1_000_000.0;
        let end_s = end as f64 / 1_000_000.0;
        args.extend([
            "-ss".to_string(),
            format!("{:.6}", start_s),
            "-to".to_string(),
            format!("{:.6}", end_s),
        ]);
        // Re-encode audio for sample-accurate cut (stream copy can only cut on keyframes)
        if has_video {
            args.extend(["-c:v".to_string(), "copy".to_string()]);
        }
        args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
        ]);
    } else {
        // Multiple segments — filter_complex with trim/atrim + concat
        let mut filter_parts = Vec::new();
        let n = segments.len();

        for (i, (start, end)) in segments.iter().enumerate() {
            let start_s = *start as f64 / 1_000_000.0;
            let end_s = *end as f64 / 1_000_000.0;

            if has_video {
                filter_parts.push(format!(
                    "[0:v]trim=start={start_s:.6}:end={end_s:.6},setpts=PTS-STARTPTS[v{i}]"
                ));
                filter_parts.push(build_audio_segment_filter(i, n, *start, *end));
            } else {
                filter_parts.push(build_audio_segment_filter(i, n, *start, *end));
            }
        }

        if has_video {
            let v_inputs: String = (0..n).map(|i| format!("[v{i}]")).collect();
            let a_inputs: String = (0..n).map(|i| format!("[a{i}]")).collect();
            filter_parts.push(format!(
                "{v_inputs}concat=n={n}:v=1:a=0[outv]; {a_inputs}concat=n={n}:v=0:a=1[outa]"
            ));
            let filter = filter_parts.join("; ");
            args.extend([
                "-filter_complex".to_string(),
                filter,
                "-map".to_string(),
                "[outv]".to_string(),
                "-map".to_string(),
                "[outa]".to_string(),
            ]);
        } else {
            let a_inputs: String = (0..n).map(|i| format!("[a{i}]")).collect();
            filter_parts.push(format!("{a_inputs}concat=n={n}:v=0:a=1[outa]"));
            let filter = filter_parts.join("; ");
            args.extend([
                "-filter_complex".to_string(),
                filter,
                "-map".to_string(),
                "[outa]".to_string(),
            ]);
        }
    }

    args.push(output_path.clone());

    log::info!("Running FFmpeg export: ffmpeg {}", args.join(" "));

    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("ffmpeg").args(&args).output()
    })
    .await
    .map_err(|e| format!("Export task panicked: {}", e))?
    .map_err(|e| {
        format!(
            "FFmpeg not found. Install FFmpeg to export edited media. Error: {}",
            e
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg export failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("FFmpeg export complete: {}", output_path);
    Ok(format!("Export complete: {}\n{}", output_path, stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_peaks_scales_to_one() {
        let peaks = vec![0.0, 0.5, 1.0, 0.25];
        let result = normalize_peaks(peaks);
        assert!((result[2] - 1.0).abs() < 0.001);
        assert!((result[1] - 0.5).abs() < 0.001);
    }

    #[test]
    fn normalize_peaks_all_zero() {
        let peaks = vec![0.0, 0.0, 0.0];
        let result = normalize_peaks(peaks);
        // global_max floor is 0.01, so all are 0/0.01 = 0
        assert!(result.iter().all(|&p| p < 0.01));
    }

    #[test]
    fn audio_segment_filter_adds_micro_fades_at_joins() {
        let filter = build_audio_segment_filter(1, 3, 1_000_000, 2_000_000);
        assert!(filter.contains("afade=t=in:st=0:d=0.008000"));
        assert!(filter.contains("afade=t=out:st=0.992000:d=0.008000"));
        assert!(filter.ends_with("[a1]"));
    }

    #[test]
    fn audio_segment_filter_scales_fade_for_short_segments() {
        let filter = build_audio_segment_filter(1, 3, 0, 6_000);
        assert!(filter.contains("afade=t=in:st=0:d=0.003000"));
        assert!(filter.contains("afade=t=out:st=0.003000:d=0.003000"));
    }
}
