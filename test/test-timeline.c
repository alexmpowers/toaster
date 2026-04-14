#include <stdio.h>

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
  toaster_time_range_t range;

  toaster_startup();

  transcript = toaster_transcript_create();
  expect_true("create transcript", transcript != NULL, "transcript should allocate");

  expect_true("add w0", toaster_transcript_add_word(transcript, "one", 0, 100000),
              "word should append");
  expect_true("add w1", toaster_transcript_add_word(transcript, "two", 100000, 200000),
              "word should append");
  expect_true("add w2", toaster_transcript_add_word(transcript, "three", 200000, 300000),
              "word should append");
  expect_true("add w3", toaster_transcript_add_word(transcript, "four", 300000, 400000),
              "word should append");
  expect_true("add w4", toaster_transcript_add_word(transcript, "five", 400000, 500000),
              "word should append");

  expect_true("delete first gap",
              toaster_transcript_delete_range(transcript, 1, 1),
              "first delete should succeed");
  expect_true("delete second gap",
              toaster_transcript_delete_range(transcript, 3, 3),
              "second delete should succeed");

  expect_true("keep segments count",
              toaster_transcript_keep_segment_count(transcript) == 3,
              "two deleted gaps should leave three keep segments");

  expect_true("segment 0",
              toaster_transcript_get_keep_segment(transcript, 0, &range) &&
                range.start_us == 0 && range.end_us == 100000,
              "first segment should cover first word");
  expect_true("segment 1",
              toaster_transcript_get_keep_segment(transcript, 1, &range) &&
                range.start_us == 200000 && range.end_us == 300000,
              "second segment should cover middle word");
  expect_true("segment 2",
              toaster_transcript_get_keep_segment(transcript, 2, &range) &&
                range.start_us == 400000 && range.end_us == 500000,
              "third segment should cover final word");

  toaster_transcript_destroy(transcript);
  toaster_shutdown();

  return failures ? 1 : 0;
}
