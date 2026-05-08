//! Extracted from the inline `mod tests` block (monolith-split).

use super::*;

fn mkword(text: &str, s: i64, e: i64) -> CanonicalWord {
    CanonicalWord {
        text: text.to_string(),
        start_us: s,
        end_us: e,
        confidence: -1.0,
        speaker_id: -1,
        is_non_speech: false,
    }
}

fn wrap(words: Vec<CanonicalWord>) -> NormalizedTranscriptionResult {
    NormalizedTranscriptionResult {
        words,
        text: String::new(),
        segments: None,
        language: "und".to_string(),
        word_timestamps_authoritative: true,
    }
}

fn tr(segments: Vec<TranscriptionSegment>) -> TranscriptionResult {
    TranscriptionResult {
        text: String::new(),
        segments: Some(segments),
    }
}

fn seg(start: f32, end: f32, text: &str) -> TranscriptionSegment {
    TranscriptionSegment {
        start,
        end,
        text: text.to_string(),
    }
}

#[test]
fn canonical_word_validates_monotonic_non_overlap() {
    assert!(wrap(vec![mkword("a", 0, 100), mkword("b", 100, 200)])
        .validate()
        .is_ok());
    assert!(
        wrap(vec![mkword("a", 0, 100), mkword("b", 200, 300)])
            .validate()
            .is_ok(),
        "gaps are allowed"
    );
    assert!(wrap(vec![mkword("a", 0, 150), mkword("b", 100, 200)])
        .validate()
        .is_err());
}

#[test]
fn canonical_word_validates_no_zero_duration() {
    assert!(wrap(vec![mkword("x", 100, 100)]).validate().is_err());
    assert!(wrap(vec![mkword("x", 100, 50)]).validate().is_err());
}

#[test]
fn canonical_word_validates_rejects_non_speech() {
    let mut w = mkword("[MUSIC]", 0, 100);
    w.is_non_speech = true;
    assert!(wrap(vec![w]).validate().is_err());
}

#[test]
fn is_non_speech_catches_hallucinations() {
    assert!(is_non_speech_token("[MUSIC]"));
    assert!(is_non_speech_token("[Applause]"));
    assert!(is_non_speech_token("<|nospeech|>"));
    assert!(is_non_speech_token("<unk>"));
    assert!(is_non_speech_token("♪♪"));
    assert!(is_non_speech_token(" ♪ ♫ ♪ "));
    assert!(is_non_speech_token("...."));
    assert!(is_non_speech_token("----"));
    assert!(!is_non_speech_token("hello"));
    assert!(!is_non_speech_token("the music was loud"));
}

#[test]
fn whisper_adapter_strips_hallucination_patterns() {
    let raw = tr(vec![
        seg(0.0, 0.5, " hello world"),
        seg(0.5, 0.8, " [MUSIC]"),
        seg(0.8, 1.0, " ♪♪"),
        seg(1.0, 1.4, " <|nospeech|>"),
        seg(1.4, 2.0, " goodbye"),
    ]);
    let audio = AudioInfo::from_samples(32_000, 16_000, 1); // 2s
    let out = WhisperAdapter.adapt(raw, audio).expect("adapt ok");
    let texts: Vec<_> = out.words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(texts, vec!["hello", "world", "goodbye"]);
    // Whisper words are char-proportional seeds — NOT authoritative until
    // DP forced alignment runs in build_words_from_segments.
    assert!(!out.word_timestamps_authoritative);
    for w in &out.words {
        assert!(!w.is_non_speech);
        assert!(w.start_us < w.end_us);
    }
}

#[test]
fn parakeet_adapter_preserves_native_word_times() {
    let raw = tr(vec![
        seg(0.10, 0.45, "hello"),
        seg(0.50, 0.80, "world"),
        seg(0.90, 1.20, "bye"),
    ]);
    let audio = AudioInfo::from_samples(32_000, 16_000, 1);
    let out = ParakeetAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(out.word_timestamps_authoritative);
    assert_eq!(out.words.len(), 3);
    assert_eq!(out.words[0].start_us, 100_000);
    assert_eq!(out.words[0].end_us, 450_000);
    assert_eq!(out.words[1].start_us, 500_000);
    assert_eq!(out.words[1].end_us, 800_000);
    assert_eq!(out.words[2].start_us, 900_000);
    assert_eq!(out.words[2].end_us, 1_200_000);
}

#[test]
fn parakeet_adapter_strips_unk_tokens() {
    let raw = tr(vec![
        seg(0.0, 0.3, "hello"),
        seg(0.3, 0.5, "<unk>"),
        seg(0.5, 0.9, "world"),
    ]);
    let audio = AudioInfo::from_samples(16_000, 16_000, 1);
    let out = ParakeetAdapter.adapt(raw, audio).expect("adapt ok");
    let texts: Vec<_> = out.words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(texts, vec!["hello", "world"]);
}

#[test]
fn whisper_language_normalization() {
    let w = WhisperAdapter;
    assert_eq!(w.normalize_language("auto"), None);
    assert_eq!(w.normalize_language(""), None);
    assert_eq!(w.normalize_language("zh-Hans").as_deref(), Some("zh"));
    assert_eq!(w.normalize_language("zh-Hant").as_deref(), Some("zh"));
    assert_eq!(w.normalize_language("en").as_deref(), Some("en"));
}

#[test]
fn moonshine_ignores_language() {
    assert_eq!(MoonshineAdapter.normalize_language("en"), None);
    assert_eq!(MoonshineAdapter.normalize_language("auto"), None);
}

#[test]
fn sense_voice_language_whitelist() {
    let sv = SenseVoiceAdapter;
    assert_eq!(sv.normalize_language("zh-Hant").as_deref(), Some("zh"));
    assert_eq!(sv.normalize_language("ja").as_deref(), Some("ja"));
    assert_eq!(sv.normalize_language("fr"), None);
}

#[test]
fn adapter_for_engine_returns_matching_impl() {
    let w = adapter_for_engine(&EngineType::Whisper);
    assert!(w.capabilities().supports_prompt_injection);
    let p = adapter_for_engine(&EngineType::Parakeet);
    assert!(!p.capabilities().supports_prompt_injection);
    assert!(p.capabilities().has_pre_speech_padding);
    assert!(p.capabilities().supports_fuzzy_word_correction);
}

#[test]
fn mock_adapter_round_trips_fixture() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mock_transcription_sample.json");
    let raw = std::fs::read_to_string(&fixture).expect("fixture exists");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("json parses");
    let segments: Vec<TranscriptionSegment> = v["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            seg(
                s["start"].as_f64().unwrap() as f32,
                s["end"].as_f64().unwrap() as f32,
                s["text"].as_str().unwrap(),
            )
        })
        .collect();
    let result = MockAdapter
        .adapt(tr(segments), AudioInfo::from_samples(16_000 * 6, 16_000, 1))
        .expect("adapt ok");
    result.validate().expect("invariants hold");
    assert!(!result.words.is_empty());
}

#[test]
fn audio_info_from_samples_computes_duration() {
    let info = AudioInfo::from_samples(32_000, 16_000, 1);
    assert_eq!(info.duration_us, 2_000_000);
}

// ── p3-abandon-even-dist-fallback ──────────────────────────────────────
//
// Adapters must refuse to produce a `NormalizedTranscriptionResult` from
// an engine that emitted text but no segment-level timings. Previously
// `commands::transcribe_file` papered over this by synthesizing
// equal-duration word timestamps downstream; that fallback has been
// removed. These tests lock in the new contract per engine.

fn raw_text_no_segments(text: &str) -> TranscriptionResult {
    TranscriptionResult {
        text: text.to_string(),
        segments: None,
    }
}

fn raw_text_empty_segments(text: &str) -> TranscriptionResult {
    TranscriptionResult {
        text: text.to_string(),
        segments: Some(Vec::new()),
    }
}

#[test]
fn every_adapter_errs_when_engine_returns_text_without_segments() {
    let audio = AudioInfo::from_samples(32_000, 16_000, 1);
    let adapters: Vec<(&str, &dyn TranscriptionModelAdapter)> = vec![
        ("Whisper", &WhisperAdapter),
        ("Parakeet", &ParakeetAdapter),
        ("Moonshine", &MoonshineAdapter),
        ("SenseVoice", &SenseVoiceAdapter),
        ("GigaAM", &GigaAmAdapter),
        ("Canary", &CanaryAdapter),
        ("Cohere", &CohereAdapter),
        ("Mock", &MockAdapter),
    ];
    for (name, a) in adapters {
        let err = a
            .adapt(raw_text_no_segments("hello world"), audio)
            .expect_err(&format!(
                "{name}: adapter must Err when engine emits text but no segments"
            ));
        let msg = err.to_string();
        assert!(
            msg.contains(name),
            "{name}: error message must name the offending engine, got {msg:?}"
        );
        assert!(
            msg.contains("equal-duration") || msg.contains("no segment"),
            "{name}: error must explain the contract violation, got {msg:?}"
        );

        // Same contract for an empty segments vec.
        assert!(
            a.adapt(raw_text_empty_segments("hello world"), audio)
                .is_err(),
            "{name}: empty segments vec with non-empty text must also Err"
        );
    }
}

#[test]
fn adapters_accept_empty_text_and_empty_segments() {
    // True silence: no text, no segments. Adapters should return an
    // empty-word result, not Err.
    let audio = AudioInfo::from_samples(32_000, 16_000, 1);
    let silent = TranscriptionResult {
        text: String::new(),
        segments: None,
    };
    let out = WhisperAdapter
        .adapt(silent, audio)
        .expect("silence is not a contract violation");
    assert!(out.words.is_empty());
}

#[test]
fn adapter_preserves_repeated_phrases() {
    // The transcript must faithfully reproduce what was spoken — even if it
    // looks like a hallucination. Dedup belongs in the cleanup flow, not here.
    // Input: "Microsoft keeps investing in Microsoft keeps investing in them"
    let raw = tr(vec![
        seg(0.0, 0.2, "Microsoft"),
        seg(0.2, 0.4, "keeps"),
        seg(0.4, 0.6, "investing"),
        seg(0.6, 0.8, "in"),
        seg(0.8, 1.0, "Microsoft"),
        seg(1.0, 1.2, "keeps"),
        seg(1.2, 1.4, "investing"),
        seg(1.4, 1.6, "in"),
        seg(1.6, 1.8, "them"),
    ]);
    let audio = AudioInfo::from_samples(32_000, 16_000, 1);
    let out = ParakeetAdapter.adapt(raw, audio).expect("adapt ok");
    let texts: Vec<_> = out.words.iter().map(|w| w.text.as_str()).collect();
    // ALL words preserved — no dedup at transcription time
    assert_eq!(
        texts,
        vec!["Microsoft", "keeps", "investing", "in", "Microsoft", "keeps", "investing", "in", "them"]
    );
}

#[test]
fn adapter_preserves_short_repeats() {
    // "very very important" — preserved as-is
    let raw = tr(vec![
        seg(0.0, 0.2, "very"),
        seg(0.2, 0.4, "very"),
        seg(0.4, 0.6, "important"),
    ]);
    let audio = AudioInfo::from_samples(16_000, 16_000, 1);
    let out = ParakeetAdapter.adapt(raw, audio).expect("adapt ok");
    let texts: Vec<_> = out.words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(texts, vec!["very", "very", "important"]);
}

#[test]
fn adapter_preserves_long_repeats() {
    // "A B C D E A B C D E F" — all preserved faithfully
    let raw = tr(vec![
        seg(0.0, 0.1, "A"),
        seg(0.1, 0.2, "B"),
        seg(0.2, 0.3, "C"),
        seg(0.3, 0.4, "D"),
        seg(0.4, 0.5, "E"),
        seg(0.5, 0.6, "A"),
        seg(0.6, 0.7, "B"),
        seg(0.7, 0.8, "C"),
        seg(0.8, 0.9, "D"),
        seg(0.9, 1.0, "E"),
        seg(1.0, 1.1, "F"),
    ]);
    let audio = AudioInfo::from_samples(32_000, 16_000, 1);
    let out = ParakeetAdapter.adapt(raw, audio).expect("adapt ok");
    let texts: Vec<_> = out.words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(texts, vec!["A", "B", "C", "D", "E", "A", "B", "C", "D", "E", "F"]);
}

/// Unit test for dedup_repeated_phrases function itself (used by cleanup flow).
#[test]
fn dedup_function_removes_repeated_phrases() {
    use crate::managers::transcription::adapter_normalize::dedup_repeated_phrases;

    let words: Vec<CanonicalWord> = ["Microsoft", "keeps", "investing", "in", "Microsoft", "keeps", "investing", "in", "them"]
        .iter()
        .enumerate()
        .map(|(i, &t)| CanonicalWord {
            text: t.to_string(),
            start_us: (i as i64) * 200_000,
            end_us: (i as i64 + 1) * 200_000,
            confidence: -1.0,
            speaker_id: -1,
            is_non_speech: false,
        })
        .collect();

    let result = dedup_repeated_phrases(words);
    let texts: Vec<_> = result.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(texts, vec!["Microsoft", "keeps", "investing", "in", "them"]);
}

// ── engine-flexibility: positive tests for undertested adapters ────────

#[test]
fn gigaam_adapter_adapts_word_level_segments() {
    let raw = tr(vec![
        seg(0.10, 0.30, "привет"),
        seg(0.35, 0.55, "мир"),
        seg(0.60, 0.90, "тест"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 2, 16_000, 1);
    let out = GigaAmAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(out.word_timestamps_authoritative, "word-level segments should be authoritative");
    assert_eq!(out.words.len(), 3);
    assert_eq!(out.words[0].text, "привет");
    assert_eq!(out.words[0].start_us, 100_000);
}

#[test]
fn gigaam_adapter_adapts_phrase_level_segments() {
    let raw = tr(vec![
        seg(0.0, 1.5, "привет мир это тест"),
        seg(1.5, 2.5, "другое предложение здесь"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 3, 16_000, 1);
    let out = GigaAmAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(!out.word_timestamps_authoritative, "phrase-level segments are NOT authoritative");
    assert!(out.words.len() >= 4, "phrase segments should split into words");
}

#[test]
fn canary_adapter_adapts_word_level_segments() {
    let raw = tr(vec![
        seg(0.05, 0.25, "hello"),
        seg(0.30, 0.50, "world"),
        seg(0.55, 0.80, "test"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 2, 16_000, 1);
    let out = CanaryAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(out.word_timestamps_authoritative);
    assert_eq!(out.words.len(), 3);
    assert_eq!(out.words[1].text, "world");
}

#[test]
fn canary_adapter_adapts_phrase_level_segments() {
    let raw = tr(vec![
        seg(0.0, 2.0, "this is a longer sentence from canary"),
        seg(2.0, 4.0, "with multiple segments here"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 5, 16_000, 1);
    let out = CanaryAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(!out.word_timestamps_authoritative);
    assert!(out.words.len() >= 6);
}

#[test]
fn cohere_adapter_adapts_word_level_segments() {
    let raw = tr(vec![
        seg(0.10, 0.35, "bonjour"),
        seg(0.40, 0.65, "le"),
        seg(0.70, 1.00, "monde"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 2, 16_000, 1);
    let out = CohereAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(out.word_timestamps_authoritative);
    assert_eq!(out.words.len(), 3);
    assert_eq!(out.words[2].text, "monde");
}

#[test]
fn cohere_adapter_adapts_phrase_level_segments() {
    let raw = tr(vec![
        seg(0.0, 1.8, "un long segment avec plusieurs mots"),
        seg(2.0, 3.5, "et un autre segment ici"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 4, 16_000, 1);
    let out = CohereAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(!out.word_timestamps_authoritative);
    assert!(out.words.len() >= 6);
}

#[test]
fn moonshine_adapter_adapts_word_level_segments() {
    let raw = tr(vec![
        seg(0.05, 0.20, "quick"),
        seg(0.25, 0.45, "brown"),
        seg(0.50, 0.70, "fox"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 2, 16_000, 1);
    let out = MoonshineAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(out.word_timestamps_authoritative);
    assert_eq!(out.words.len(), 3);
}

#[test]
fn sensevoice_adapter_adapts_word_level_segments() {
    let raw = tr(vec![
        seg(0.10, 0.30, "你好"),
        seg(0.35, 0.55, "世界"),
        seg(0.60, 0.85, "测试"),
    ]);
    let audio = AudioInfo::from_samples(16_000 * 2, 16_000, 1);
    let out = SenseVoiceAdapter.adapt(raw, audio).expect("adapt ok");
    assert!(out.word_timestamps_authoritative);
    assert_eq!(out.words.len(), 3);
}

// ── sanitize_segments tests ─────────────────────────────────────────
// `sanitize_segments` is imported via `use super::*` from adapter.rs

#[test]
fn sanitize_segments_clamps_overlapping() {
    let segs = vec![
        seg(0.0, 1.0, "hello"),
        seg(0.8, 2.0, "world"),   // overlaps by 0.2s
        seg(2.0, 3.0, "again"),
    ];
    let result = sanitize_segments(&segs);
    assert_eq!(result.len(), 3);
    // second segment start clamped to first's end
    assert_eq!(result[1].start, 1.0);
    assert_eq!(result[1].end, 2.0);
    // third unchanged
    assert_eq!(result[2].start, 2.0);
}

#[test]
fn sanitize_segments_drops_fully_contained() {
    let segs = vec![
        seg(0.0, 3.0, "long segment"),
        seg(1.0, 2.0, "contained"),  // fully inside, becomes zero after clamping
        seg(3.0, 4.0, "after"),
    ];
    let result = sanitize_segments(&segs);
    // "contained" seg starts at 0.0→clamped to 3.0, end=2.0 < 3.0 → dropped
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].text, "long segment");
    assert_eq!(result[1].text, "after");
}

#[test]
fn sanitize_segments_strips_non_speech() {
    let segs = vec![
        seg(0.0, 1.0, "hello"),
        seg(1.0, 2.0, "[Music]"),
        seg(2.0, 3.0, ""),
        seg(3.0, 4.0, "  "),
        seg(4.0, 5.0, "world"),
    ];
    let result = sanitize_segments(&segs);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].text, "hello");
    assert_eq!(result[1].text, "world");
}

#[test]
fn sanitize_segments_preserves_clean_input() {
    let segs = vec![
        seg(0.0, 1.0, "one"),
        seg(1.0, 2.0, "two"),
        seg(2.5, 3.5, "three"),  // gap is fine
    ];
    let result = sanitize_segments(&segs);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].start, 0.0);
    assert_eq!(result[2].start, 2.5);
}

#[test]
fn sanitize_segments_empty_input() {
    let result = sanitize_segments(&[]);
    assert!(result.is_empty());
}
