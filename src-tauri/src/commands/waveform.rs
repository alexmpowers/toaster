use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::editor::Word;

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
pub fn generate_waveform_peaks(path: String, peak_count: Option<usize>) -> Result<Vec<f32>, String> {
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
}
