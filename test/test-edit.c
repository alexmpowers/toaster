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
  toaster_word_t word;
  toaster_time_range_t range;

  toaster_startup();

  transcript = toaster_transcript_create();
  expect_true("create transcript", transcript != NULL, "transcript should allocate");

  expect_true("startup active", toaster_is_started(), "startup flag should be active");
  expect_true("add word hello",
              toaster_transcript_add_word(transcript, "hello", 0, 450000),
              "first word should append");
  expect_true("add word um",
              toaster_transcript_add_word(transcript, "um", 450000, 620000),
              "second word should append");
  expect_true("add word welcome",
              toaster_transcript_add_word(transcript, "welcome", 620000, 1200000),
              "third word should append");
  expect_true("word count",
              toaster_transcript_word_count(transcript) == 3,
              "transcript should expose three words");

  expect_true("delete middle word",
              toaster_transcript_delete_range(transcript, 1, 1),
              "delete should succeed");
  expect_true("read deleted word",
              toaster_transcript_get_word(transcript, 1, &word),
              "deleted word should still be readable");
  expect_true("deleted flag", word.deleted, "deleted word should be marked");
  expect_true("word text", strcmp(word.text, "um") == 0, "word text should stay intact");

  expect_true("deleted span count",
              toaster_transcript_deleted_span_count(transcript) == 1,
              "one deleted span expected");
  expect_true("deleted span range",
              toaster_transcript_get_deleted_span(transcript, 0, &range) &&
                range.start_us == 450000 && range.end_us == 620000,
              "deleted span timestamps should match deleted word");

  expect_true("keep segment count",
              toaster_transcript_keep_segment_count(transcript) == 2,
              "deleting one word should create two keep segments");
  expect_true("first keep segment",
              toaster_transcript_get_keep_segment(transcript, 0, &range) &&
                range.start_us == 0 && range.end_us == 450000,
              "first keep segment should end before filler");
  expect_true("second keep segment",
              toaster_transcript_get_keep_segment(transcript, 1, &range) &&
                range.start_us == 620000 && range.end_us == 1200000,
              "second keep segment should resume after filler");

  expect_true("restore all",
              toaster_transcript_restore_all(transcript),
              "restore all should succeed");
  expect_true("single keep segment after restore",
              toaster_transcript_keep_segment_count(transcript) == 1 &&
                toaster_transcript_get_keep_segment(transcript, 0, &range) &&
                range.start_us == 0 && range.end_us == 1200000,
              "restore should recover one continuous segment");

  toaster_transcript_destroy(transcript);
  toaster_shutdown();

  return failures ? 1 : 0;
}
