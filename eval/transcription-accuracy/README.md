# Transcription Accuracy Eval

Measures word-timestamp accuracy by comparing `.toaster` project transcription
results against hand-labeled oracle timestamps.

## How it works

1. A `.toaster` project file contains the ASR pipeline's word timestamps
2. An oracle JSON file contains human-verified ground-truth timestamps
3. The eval script compares each word's `start_us`/`end_us` against the oracle
4. Reports median error, p95 error, max error, and word-match rate
5. Pass/fail is determined by tolerance thresholds in the fixture

## Fixture format

Each fixture is a JSON file in `fixtures/` following `schema.json`:

```json
{
  "id": "toaster_example_parakeet_v3",
  "source_project": "../../fixtures/toaster_example.mp4.toaster",
  "engine": "parakeet-tdt-0.6b-v3",
  "words": [
    {"text": "Yeah,", "oracle_start_us": 390000, "oracle_end_us": 650000},
    ...
  ],
  "tolerances": {
    "median_error_us": 100000,
    "p95_error_us": 500000
  }
}
```

## Creating oracle fixtures

1. Open the audio in an editor (Audacity recommended) with waveform + spectrogram
2. For each word in the `.toaster` project, identify speech onset and offset
3. Record `oracle_start_us` (first voicing) and `oracle_end_us` (last voicing)
4. Save as `fixtures/<id>.json`

## Running

```powershell
scripts/eval/eval-transcription-accuracy.ps1
```

Or via the eval harness:

```powershell
scripts/eval/run-eval-harness.ps1
```
