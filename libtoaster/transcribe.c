#include "toaster.h"

#ifdef TOASTER_HAS_WHISPER

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <whisper.h>

static volatile bool transcribe_cancel_flag = false;

/* whisper.cpp progress callback adapter */
static void whisper_progress_adapter(struct whisper_context *ctx, struct whisper_state *state, int progress,
                                     void *user_data)
{
  (void)ctx;
  (void)state;
  void **cb_pair = (void **)user_data;
  toaster_transcribe_progress_cb cb = (toaster_transcribe_progress_cb)cb_pair[0];
  void *ud = cb_pair[1];

  if (cb)
    cb(progress, ud);
}

/* whisper.cpp abort callback — returns true to abort */
static bool whisper_abort_adapter(void *user_data)
{
  (void)user_data;
  return transcribe_cancel_flag;
}

/* Strip leading/trailing whitespace from a token text, return trimmed copy */
static char *trim_token(const char *text)
{
  const char *start, *end;
  size_t len;
  char *result;

  if (!text)
    return NULL;

  start = text;
  while (*start == ' ' || *start == '\t')
    start++;

  end = text + strlen(text);
  while (end > start && (*(end - 1) == ' ' || *(end - 1) == '\t'))
    end--;

  len = (size_t)(end - start);
  if (len == 0)
    return NULL;

  result = (char *)malloc(len + 1);
  if (!result)
    return NULL;

  memcpy(result, start, len);
  result[len] = '\0';
  return result;
}

bool toaster_transcribe(toaster_transcript_t *transcript, const float *pcm_samples, size_t sample_count,
                        int sample_rate, const char *language,
                        toaster_transcribe_progress_cb progress_cb, void *user_data)
{
  const char *model_path;
  struct whisper_context_params cparams;
  struct whisper_context *ctx = NULL;
  struct whisper_full_params wparams;
  int n_segments, seg, tok, n_tokens;
  void *cb_pair[2];
  bool success = false;

  if (!transcript || !pcm_samples || sample_count == 0)
    return false;

  /* Get model path for the active model */
  model_path = toaster_model_get_path(toaster_model_get_active());
  if (!model_path) {
    fprintf(stderr, "toaster_transcribe: no model available (active=%s)\n", toaster_model_get_active());
    return false;
  }

  /* Initialize whisper context */
  cparams = whisper_context_default_params();
  ctx = whisper_init_from_file_with_params(model_path, cparams);
  if (!ctx) {
    fprintf(stderr, "toaster_transcribe: failed to load model: %s\n", model_path);
    return false;
  }

  /* Configure transcription parameters */
  wparams = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
  wparams.print_progress = false;
  wparams.print_special = false;
  wparams.print_realtime = false;
  wparams.print_timestamps = false;
  wparams.token_timestamps = true;
  wparams.max_tokens = 32;
  wparams.language = language ? language : "en";
  wparams.n_threads = 4;

  /* Set up progress callback */
  cb_pair[0] = (void *)progress_cb;
  cb_pair[1] = user_data;
  wparams.progress_callback = whisper_progress_adapter;
  wparams.progress_callback_user_data = cb_pair;
  wparams.abort_callback = whisper_abort_adapter;
  wparams.abort_callback_user_data = NULL;

  transcribe_cancel_flag = false;

  /* Run transcription */
  if (whisper_full(ctx, wparams, pcm_samples, (int)sample_count) != 0) {
    fprintf(stderr, "toaster_transcribe: whisper_full failed\n");
    goto cleanup;
  }

  if (transcribe_cancel_flag)
    goto cleanup;

  /* Extract word-level results */
  n_segments = whisper_full_n_segments(ctx);
  for (seg = 0; seg < n_segments; seg++) {
    n_tokens = whisper_full_n_tokens(ctx, seg);
    for (tok = 0; tok < n_tokens; tok++) {
      whisper_token_data tdata = whisper_full_get_token_data(ctx, seg, tok);
      const char *token_text = whisper_full_get_token_text(ctx, seg, tok);
      char *trimmed;
      int64_t t0_us, t1_us;
      size_t word_idx;

      /* Skip special tokens (timestamps, etc.) */
      if (!token_text || token_text[0] == '[')
        continue;

      trimmed = trim_token(token_text);
      if (!trimmed)
        continue;

      /* whisper timestamps are in centiseconds (10ms units), convert to microseconds */
      t0_us = tdata.t0 * 10000LL;
      t1_us = tdata.t1 * 10000LL;

      /* Ensure valid time range */
      if (t1_us <= t0_us)
        t1_us = t0_us + 10000LL; /* minimum 10ms */

      if (!toaster_transcript_add_word(transcript, trimmed, t0_us, t1_us)) {
        free(trimmed);
        goto cleanup;
      }

      /* Set confidence from token probability */
      word_idx = toaster_transcript_word_count(transcript) - 1;
      toaster_transcript_set_word_confidence(transcript, word_idx, tdata.p);

      free(trimmed);
    }
  }

  success = true;

cleanup:
  whisper_free(ctx);
  return success;
}

bool toaster_transcribe_cancel(void)
{
  transcribe_cancel_flag = true;
  return true;
}

#else /* !TOASTER_HAS_WHISPER */

#include <stdio.h>

bool toaster_transcribe(toaster_transcript_t *transcript, const float *pcm_samples, size_t sample_count,
                        int sample_rate, const char *language,
                        toaster_transcribe_progress_cb progress_cb, void *user_data)
{
  (void)transcript;
  (void)pcm_samples;
  (void)sample_count;
  (void)sample_rate;
  (void)language;
  (void)progress_cb;
  (void)user_data;
  fprintf(stderr, "toaster_transcribe: whisper.cpp not available (built without TOASTER_HAS_WHISPER)\n");
  return false;
}

bool toaster_transcribe_cancel(void)
{
  return false;
}

#endif /* TOASTER_HAS_WHISPER */
