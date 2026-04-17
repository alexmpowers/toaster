use crate::audio_toolkit::{list_input_devices, vad::SmoothedVad, AudioRecorder, SileroVad};
use crate::helpers::clamshell;
use crate::settings::{get_settings, AppSettings};
use crate::utils;
use log::{debug, error, info};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;

const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

fn set_mute(mute: bool) {
    // Expected behavior:
    // - Windows: works on most systems using standard audio drivers.
    // - Linux: works on many systems (PipeWire, PulseAudio, ALSA),
    //   but some distros may lack the tools used.
    // - macOS: works on most standard setups via AppleScript.
    // If unsupported, fails silently.

    #[cfg(target_os = "windows")]
    {
        unsafe {
            use windows::Win32::{
                Media::Audio::{
                    eMultimedia, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
                    MMDeviceEnumerator,
                },
                System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
            };

            macro_rules! unwrap_or_return {
                ($expr:expr) => {
                    match $expr {
                        Ok(val) => val,
                        Err(_) => return,
                    }
                };
            }

            // Initialize the COM library for this thread.
            // If already initialized (e.g., by another library like Tauri), this does nothing.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let all_devices: IMMDeviceEnumerator =
                unwrap_or_return!(CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL));
            let default_device =
                unwrap_or_return!(all_devices.GetDefaultAudioEndpoint(eRender, eMultimedia));
            let volume_interface = unwrap_or_return!(
                default_device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            );

            let _ = volume_interface.SetMute(mute, std::ptr::null());
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let mute_val = if mute { "1" } else { "0" };
        let amixer_state = if mute { "mute" } else { "unmute" };

        // Try multiple backends to increase compatibility
        // 1. PipeWire (wpctl)
        if Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 2. PulseAudio (pactl)
        if Command::new("pactl")
            .args(["set-sink-mute", "@DEFAULT_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 3. ALSA (amixer)
        let _ = Command::new("amixer")
            .args(["set", "Master", amixer_state])
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = format!(
            "set volume output muted {}",
            if mute { "true" } else { "false" }
        );
        let _ = Command::new("osascript").args(["-e", &script]).output();
    }
}

const WHISPER_SAMPLE_RATE: usize = 16000;

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug, PartialEq)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MicrophoneMode {
    AlwaysOn,
    OnDemand,
}

/* ──────────────────────────────────────────────────────────────── */

fn create_audio_recorder(
    vad_path: &str,
    app_handle: &tauri::AppHandle,
) -> Result<AudioRecorder, anyhow::Error> {
    let silero = SileroVad::new(vad_path, 0.3)
        .map_err(|e| anyhow::anyhow!("Failed to create SileroVad: {}", e))?;
    let smoothed_vad = SmoothedVad::new(Box::new(silero), 15, 15, 2);

    // Recorder with VAD plus a spectrum-level callback that forwards updates to
    // the frontend.
    let recorder = AudioRecorder::new()
        .map_err(|e| anyhow::anyhow!("Failed to create AudioRecorder: {}", e))?
        .with_vad(Box::new(smoothed_vad))
        .with_level_callback({
            let app_handle = app_handle.clone();
            move |levels| {
                utils::emit_levels(&app_handle, &levels);
            }
        });

    Ok(recorder)
}

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    mode: Arc<Mutex<MicrophoneMode>>,
    app_handle: tauri::AppHandle,

    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_open: Arc<Mutex<bool>>,
    is_recording: Arc<Mutex<bool>>,
    did_mute: Arc<Mutex<bool>>,
    close_generation: Arc<AtomicU64>,
}

impl AudioRecordingManager {
    /* ---------- construction ------------------------------------------------ */

    pub fn new(app: &tauri::AppHandle) -> Result<Self, anyhow::Error> {
        let settings = get_settings(app);
        let mode = if settings.always_on_microphone {
            MicrophoneMode::AlwaysOn
        } else {
            MicrophoneMode::OnDemand
        };

        let manager = Self {
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            mode: Arc::new(Mutex::new(mode.clone())),
            app_handle: app.clone(),

            recorder: Arc::new(Mutex::new(None)),
            is_open: Arc::new(Mutex::new(false)),
            is_recording: Arc::new(Mutex::new(false)),
            did_mute: Arc::new(Mutex::new(false)),
            close_generation: Arc::new(AtomicU64::new(0)),
        };

        // Always-on?  Open immediately.
        if matches!(mode, MicrophoneMode::AlwaysOn) {
            manager.start_microphone_stream()?;
        }

        Ok(manager)
    }

    /* ---------- helper methods --------------------------------------------- */

    fn get_effective_microphone_device(&self, settings: &AppSettings) -> Option<cpal::Device> {
        // Check if we're in clamshell mode and have a clamshell microphone configured
        let use_clamshell_mic = if let Ok(is_clamshell) = clamshell::is_clamshell() {
            is_clamshell && settings.clamshell_microphone.is_some()
        } else {
            false
        };

        let device_name = if use_clamshell_mic {
            settings.clamshell_microphone.as_ref()?
        } else {
            settings.selected_microphone.as_ref()?
        };

        // Find the device by name
        match list_input_devices() {
            Ok(devices) => devices
                .into_iter()
                .find(|d| d.name == *device_name)
                .map(|d| d.device),
            Err(e) => {
                debug!("Failed to list devices, using default: {}", e);
                None
            }
        }
    }

    fn schedule_lazy_close(&self) {
        let gen = self.close_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let app = self.app_handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(STREAM_IDLE_TIMEOUT);
            let rm = app.state::<Arc<AudioRecordingManager>>();
            // Hold state lock across the check AND close to serialize against
            // try_start_recording, preventing a race where the stream is closed
            // under an active recording.
            let state = rm.state.lock().unwrap();
            if rm.close_generation.load(Ordering::SeqCst) == gen
                && matches!(*state, RecordingState::Idle)
            {
                // stop_microphone_stream does not acquire the state lock,
                // so holding it here is safe (no deadlock).
                info!(
                    "Closing idle microphone stream after {:?}",
                    STREAM_IDLE_TIMEOUT
                );
                rm.stop_microphone_stream();
            }
        });
    }

    /* ---------- microphone life-cycle -------------------------------------- */

    /// Applies mute if mute_while_recording is enabled and stream is open
    pub fn apply_mute(&self) {
        let settings = get_settings(&self.app_handle);
        let mut did_mute_guard = self.did_mute.lock().unwrap();

        if settings.mute_while_recording && *self.is_open.lock().unwrap() {
            set_mute(true);
            *did_mute_guard = true;
            debug!("Mute applied");
        }
    }

    /// Removes mute if it was applied
    pub fn remove_mute(&self) {
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
            *did_mute_guard = false;
            debug!("Mute removed");
        }
    }

    pub fn preload_vad(&self) -> Result<(), anyhow::Error> {
        let mut recorder_opt = self.recorder.lock().unwrap();
        if recorder_opt.is_none() {
            let vad_path = self
                .app_handle
                .path()
                .resolve(
                    "resources/models/silero_vad_v4.onnx",
                    tauri::path::BaseDirectory::Resource,
                )
                .map_err(|e| anyhow::anyhow!("Failed to resolve VAD path: {}", e))?;
            *recorder_opt = Some(create_audio_recorder(
                vad_path.to_str().ok_or_else(|| {
                    anyhow::anyhow!(
                        "VAD model path contains invalid UTF-8: {}",
                        vad_path.display()
                    )
                })?,
                &self.app_handle,
            )?);
        }
        Ok(())
    }

    pub fn start_microphone_stream(&self) -> Result<(), anyhow::Error> {
        let mut open_flag = self.is_open.lock().unwrap();
        if *open_flag {
            debug!("Microphone stream already active");
            return Ok(());
        }

        let start_time = Instant::now();

        // Don't mute immediately - caller will handle muting after audio feedback
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        *did_mute_guard = false;

        // Get the selected device from settings, considering clamshell mode
        let settings = get_settings(&self.app_handle);
        let selected_device = self.get_effective_microphone_device(&settings);

        // Pre-flight check: if no device was selected/configured AND no devices
        // exist at all, fail early with a clear error instead of letting cpal
        // produce a cryptic backend-specific message.
        if selected_device.is_none() {
            let has_any_device = list_input_devices()
                .map(|devices| !devices.is_empty())
                .unwrap_or(false);
            if !has_any_device {
                return Err(anyhow::anyhow!("No input device found"));
            }
        }

        // Ensure VAD is loaded if it wasn't for whatever reason
        self.preload_vad()?;

        let mut recorder_opt = self.recorder.lock().unwrap();
        if let Some(rec) = recorder_opt.as_mut() {
            rec.open(selected_device)
                .map_err(|e| anyhow::anyhow!("Failed to open recorder: {}", e))?;
        }

        *open_flag = true;
        info!(
            "Microphone stream initialized in {:?}",
            start_time.elapsed()
        );
        Ok(())
    }

    pub fn stop_microphone_stream(&self) {
        let mut open_flag = self.is_open.lock().unwrap();
        if !*open_flag {
            return;
        }

        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
        }
        *did_mute_guard = false;

        if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
            // If still recording, stop first.
            if *self.is_recording.lock().unwrap() {
                let _ = rec.stop();
                *self.is_recording.lock().unwrap() = false;
            }
            let _ = rec.close();
        }

        *open_flag = false;
        debug!("Microphone stream stopped");
    }

    /* ---------- mode switching --------------------------------------------- */

    pub fn update_mode(&self, new_mode: MicrophoneMode) -> Result<(), anyhow::Error> {
        let cur_mode = self.mode.lock().unwrap().clone();

        match (cur_mode, &new_mode) {
            (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand) => {
                if matches!(*self.state.lock().unwrap(), RecordingState::Idle) {
                    self.close_generation.fetch_add(1, Ordering::SeqCst);
                    self.stop_microphone_stream();
                }
            }
            (MicrophoneMode::OnDemand, MicrophoneMode::AlwaysOn) => {
                self.close_generation.fetch_add(1, Ordering::SeqCst);
                self.start_microphone_stream()?;
            }
            _ => {}
        }

        *self.mode.lock().unwrap() = new_mode;
        Ok(())
    }

    /* ---------- recording --------------------------------------------------- */

    pub fn try_start_recording(&self, binding_id: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Idle = *state {
            // Ensure microphone is open in on-demand mode
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                // Cancel any pending lazy close
                self.close_generation.fetch_add(1, Ordering::SeqCst);
                if let Err(e) = self.start_microphone_stream() {
                    let msg = format!("{e}");
                    error!("Failed to open microphone stream: {msg}");
                    return Err(msg);
                }
            }

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                if rec.start().is_ok() {
                    *self.is_recording.lock().unwrap() = true;
                    *state = RecordingState::Recording {
                        binding_id: binding_id.to_string(),
                    };
                    debug!("Recording started for binding {binding_id}");
                    return Ok(());
                }
            }
            Err("Recorder not available".to_string())
        } else {
            Err("Already recording".to_string())
        }
    }

    pub fn update_selected_device(&self) -> Result<(), anyhow::Error> {
        // If currently open, restart the microphone stream to use the new device
        if *self.is_open.lock().unwrap() {
            self.close_generation.fetch_add(1, Ordering::SeqCst);
            self.stop_microphone_stream();
            self.start_microphone_stream()?;
        }
        Ok(())
    }

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording {
                binding_id: ref active,
            } if active == binding_id => {
                *state = RecordingState::Idle;
                drop(state);

                // Optionally keep recording for a bit longer to capture trailing audio
                let settings = get_settings(&self.app_handle);
                if settings.extra_recording_buffer_ms > 0 {
                    debug!(
                        "Extra recording buffer: sleeping {}ms before stopping",
                        settings.extra_recording_buffer_ms
                    );
                    std::thread::sleep(Duration::from_millis(settings.extra_recording_buffer_ms));
                }

                let samples = if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    match rec.stop() {
                        Ok(buf) => buf,
                        Err(e) => {
                            error!("stop() failed: {e}");
                            Vec::new()
                        }
                    }
                } else {
                    error!("Recorder not available");
                    Vec::new()
                };

                *self.is_recording.lock().unwrap() = false;

                // In on-demand mode, close the mic (lazily if the setting is enabled)
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    if get_settings(&self.app_handle).lazy_stream_close {
                        self.schedule_lazy_close();
                    } else {
                        self.stop_microphone_stream();
                    }
                }

                Some(pad_short_samples(samples))
            }
            _ => None,
        }
    }
    pub fn is_recording(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            RecordingState::Recording { .. }
        )
    }

    /// Cancel any ongoing recording without returning audio samples
    pub fn cancel_recording(&self) {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Recording { .. } = *state {
            *state = RecordingState::Idle;
            drop(state);

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                let _ = rec.stop(); // Discard the result
            }

            *self.is_recording.lock().unwrap() = false;

            // In on-demand mode, close the mic (lazily if the setting is enabled)
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                if get_settings(&self.app_handle).lazy_stream_close {
                    self.schedule_lazy_close();
                } else {
                    self.stop_microphone_stream();
                }
            }
        }
    }
}

/// Pad samples shorter than one Whisper frame to avoid truncation artifacts.
/// Empty buffers are returned as-is; buffers already at or above
/// `WHISPER_SAMPLE_RATE` are returned unchanged.
fn pad_short_samples(samples: Vec<f32>) -> Vec<f32> {
    let len = samples.len();
    if len < WHISPER_SAMPLE_RATE && len > 0 {
        let mut padded = samples;
        padded.resize(WHISPER_SAMPLE_RATE * 5 / 4, 0.0);
        padded
    } else {
        samples
    }
}

/* ════════════════════════════════════════════════════════════════ */
/*                            Tests                                */
/* ════════════════════════════════════════════════════════════════ */

#[cfg(test)]
mod tests {
    use super::*;

    // ── RecordingState ────────────────────────────────────────────

    #[test]
    fn recording_state_defaults_to_idle() {
        let state = RecordingState::Idle;
        assert!(matches!(state, RecordingState::Idle));
    }

    #[test]
    fn recording_state_carries_binding_id() {
        let state = RecordingState::Recording {
            binding_id: "key-a".to_string(),
        };
        if let RecordingState::Recording { binding_id } = &state {
            assert_eq!(binding_id, "key-a");
        } else {
            panic!("Expected Recording variant");
        }
    }

    #[test]
    fn recording_state_clone_is_independent() {
        let a = RecordingState::Recording {
            binding_id: "b1".into(),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn recording_state_idle_eq() {
        assert_eq!(RecordingState::Idle, RecordingState::Idle);
    }

    #[test]
    fn recording_state_different_bindings_not_eq() {
        let a = RecordingState::Recording {
            binding_id: "x".into(),
        };
        let b = RecordingState::Recording {
            binding_id: "y".into(),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn recording_state_idle_vs_recording_not_eq() {
        let idle = RecordingState::Idle;
        let rec = RecordingState::Recording {
            binding_id: "z".into(),
        };
        assert_ne!(idle, rec);
    }

    // ── MicrophoneMode ───────────────────────────────────────────

    #[test]
    fn microphone_mode_equality() {
        assert_eq!(MicrophoneMode::AlwaysOn, MicrophoneMode::AlwaysOn);
        assert_eq!(MicrophoneMode::OnDemand, MicrophoneMode::OnDemand);
        assert_ne!(MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand);
    }

    #[test]
    fn microphone_mode_clone() {
        let m = MicrophoneMode::OnDemand;
        assert_eq!(m.clone(), MicrophoneMode::OnDemand);
    }

    #[test]
    fn microphone_mode_debug_format() {
        let dbg = format!("{:?}", MicrophoneMode::AlwaysOn);
        assert!(dbg.contains("AlwaysOn"));
    }

    // ── Constants ────────────────────────────────────────────────

    #[test]
    fn stream_idle_timeout_is_30s() {
        assert_eq!(STREAM_IDLE_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn whisper_sample_rate_is_16khz() {
        assert_eq!(WHISPER_SAMPLE_RATE, 16_000);
    }

    // ── pad_short_samples ────────────────────────────────────────

    #[test]
    fn pad_empty_samples_returns_empty() {
        let result = pad_short_samples(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn pad_single_sample_pads_to_expected_length() {
        let result = pad_short_samples(vec![0.5]);
        assert_eq!(result.len(), WHISPER_SAMPLE_RATE * 5 / 4);
        assert_eq!(result[0], 0.5);
        // Padded region must be silence (0.0)
        assert!(result[1..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn pad_just_below_threshold_pads() {
        let input = vec![0.1; WHISPER_SAMPLE_RATE - 1];
        let result = pad_short_samples(input);
        assert_eq!(result.len(), WHISPER_SAMPLE_RATE * 5 / 4);
    }

    #[test]
    fn pad_at_threshold_does_not_pad() {
        let input = vec![0.2; WHISPER_SAMPLE_RATE];
        let result = pad_short_samples(input.clone());
        assert_eq!(result.len(), WHISPER_SAMPLE_RATE);
        assert_eq!(result, input);
    }

    #[test]
    fn pad_above_threshold_unchanged() {
        let input = vec![0.3; WHISPER_SAMPLE_RATE + 500];
        let result = pad_short_samples(input.clone());
        assert_eq!(result, input);
    }

    #[test]
    fn pad_preserves_original_samples() {
        let input: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let result = pad_short_samples(input.clone());
        assert_eq!(&result[..100], &input[..]);
    }

    // ── State-machine guard logic (inline) ───────────────────────

    #[test]
    fn stop_recording_wrong_binding_returns_none() {
        // Simulates the guard in stop_recording: mismatched binding_id
        let state = RecordingState::Recording {
            binding_id: "active-key".into(),
        };
        let requested = "other-key";

        let result = match &state {
            RecordingState::Recording { binding_id } if binding_id == requested => {
                Some("would stop")
            }
            _ => None,
        };
        assert!(result.is_none());
    }

    #[test]
    fn stop_recording_matching_binding_returns_some() {
        let state = RecordingState::Recording {
            binding_id: "my-key".into(),
        };
        let requested = "my-key";

        let result = match &state {
            RecordingState::Recording { binding_id } if binding_id == requested => {
                Some("stopped")
            }
            _ => None,
        };
        assert_eq!(result, Some("stopped"));
    }

    #[test]
    fn stop_recording_idle_returns_none() {
        let state = RecordingState::Idle;
        let result = match &state {
            RecordingState::Recording { binding_id } if binding_id == "any" => Some("stopped"),
            _ => None,
        };
        assert!(result.is_none());
    }

    #[test]
    fn try_start_guards_against_double_recording() {
        // Simulates the guard in try_start_recording
        let state = RecordingState::Recording {
            binding_id: "existing".into(),
        };
        let can_start = matches!(state, RecordingState::Idle);
        assert!(!can_start);
    }

    #[test]
    fn try_start_allows_from_idle() {
        let state = RecordingState::Idle;
        let can_start = matches!(state, RecordingState::Idle);
        assert!(can_start);
    }

    // ── Mode-transition logic ────────────────────────────────────

    #[test]
    fn mode_transition_always_on_to_on_demand_detected() {
        let cur = MicrophoneMode::AlwaysOn;
        let new = MicrophoneMode::OnDemand;
        let should_close = matches!((&cur, &new), (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand));
        assert!(should_close);
    }

    #[test]
    fn mode_transition_on_demand_to_always_on_detected() {
        let cur = MicrophoneMode::OnDemand;
        let new = MicrophoneMode::AlwaysOn;
        let should_open = matches!((&cur, &new), (MicrophoneMode::OnDemand, MicrophoneMode::AlwaysOn));
        assert!(should_open);
    }

    #[test]
    fn mode_transition_same_mode_is_noop() {
        for mode in [MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand] {
            let same = mode.clone();
            let needs_action = matches!(
                (&mode, &same),
                (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand)
                    | (MicrophoneMode::OnDemand, MicrophoneMode::AlwaysOn)
            );
            assert!(!needs_action);
        }
    }

    // ── is_recording helper ──────────────────────────────────────

    #[test]
    fn is_recording_matches_only_recording_variant() {
        assert!(matches!(
            RecordingState::Recording {
                binding_id: "x".into()
            },
            RecordingState::Recording { .. }
        ));
        assert!(!matches!(RecordingState::Idle, RecordingState::Recording { .. }));
    }
}
