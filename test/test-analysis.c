#include <stdio.h>
#include <string.h>

#include "toaster.h"

static int failures = 0;

#define PASS(name) printf("  PASS: %s\n", name)
#define FAIL(name, msg)                                                              \
  do {                                                                               \
    printf("  FAIL: %s - %s\n", name, msg);                                          \
    failures++;                                                                      \
  } while (0)

static void expect_true(const char *name, bool condition, const char *message)
{
  if (condition)
    PASS(name);
  else
    FAIL(name, message);
}

int main(void)
{
  toaster_transcript_t *transcript;
  toaster_suggestion_list_t *fillers;
  toaster_suggestion_list_t *pauses;
  toaster_suggestion_t suggestion;

  toaster_startup();

  transcript = toaster_transcript_create();
  fillers = toaster_suggestion_list_create();
  pauses = toaster_suggestion_list_create();

  expect_true("create transcript", transcript != NULL, "transcript should allocate");
  expect_true("create filler list", fillers != NULL, "filler list should allocate");
  expect_true("create pause list", pauses != NULL, "pause list should allocate");

  expect_true("add um", toaster_transcript_add_word(transcript, "um", 0, 100000),
              "word should append");
  expect_true("add you", toaster_transcript_add_word(transcript, "you", 100000, 200000),
              "word should append");
  expect_true("add know", toaster_transcript_add_word(transcript, "know", 200000, 300000),
              "word should append");
  expect_true("add ship", toaster_transcript_add_word(transcript, "ship", 900000, 1000000),
              "word should append");
  expect_true("add ship 2", toaster_transcript_add_word(transcript, "ship", 1000000, 1100000),
              "word should append");

  expect_true("detect fillers",
              toaster_detect_fillers(transcript, fillers),
              "filler analysis should succeed");
  expect_true("filler count",
              toaster_suggestion_list_count(fillers) == 3,
              "expected single filler, phrase filler, and repeated word");
  expect_true("phrase filler suggestion",
              toaster_suggestion_list_get(fillers, 1, &suggestion) &&
                suggestion.start_index == 1 && suggestion.end_index == 2 &&
                strcmp(suggestion.reason, "Phrase filler") == 0,
              "phrase filler should cover you know");

  expect_true("detect pauses",
              toaster_detect_pauses(transcript, pauses, 300000, 100000),
              "pause analysis should succeed");
  expect_true("pause count",
              toaster_suggestion_list_count(pauses) == 1,
              "one large pause expected");
  expect_true("pause suggestion",
              toaster_suggestion_list_get(pauses, 0, &suggestion) &&
                suggestion.kind == TOASTER_SUGGESTION_SHORTEN_PAUSE &&
                suggestion.start_us == 300000 && suggestion.end_us == 900000 &&
                suggestion.replacement_duration_us == 100000,
              "pause suggestion should capture source gap");

  expect_true("apply pause cut",
              toaster_transcript_add_cut_span(transcript, 300000, 800000),
              "pause cut should append");
  toaster_suggestion_list_clear(pauses);
  expect_true("detect pauses after cut",
              toaster_detect_pauses(transcript, pauses, 300000, 100000),
              "pause analysis should still succeed after cut");
  expect_true("pause count after cut",
              toaster_suggestion_list_count(pauses) == 0,
              "already-cut pauses should not be suggested again");

  toaster_suggestion_list_destroy(fillers);
  toaster_suggestion_list_destroy(pauses);
  toaster_transcript_destroy(transcript);
  toaster_shutdown();

  return failures ? 1 : 0;
}
