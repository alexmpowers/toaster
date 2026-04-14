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
  const char *path = "test-project.toaster";
  toaster_project_t *project;
  toaster_project_t *loaded;
  toaster_word_t word;
  toaster_time_range_t range;
  const toaster_transcript_t *loaded_transcript;

  toaster_startup();

  project = toaster_project_create();
  expect_true("create project", project != NULL, "project should allocate");
  expect_true("set media path",
              toaster_project_set_media_path(project, "C:\\Media\\sample.mp4"),
              "media path should save");
  expect_true("set language",
              toaster_project_set_language(project, "en-US"),
              "language should save");
  expect_true("add first word",
              toaster_transcript_add_word(toaster_project_get_transcript(project), "Add", 0, 500000),
              "first word should append");
  expect_true("add second word",
              toaster_transcript_add_word(toaster_project_get_transcript(project), "um", 500000, 750000),
              "second word should append");
  expect_true("delete second word",
              toaster_transcript_delete_range(toaster_project_get_transcript(project), 1, 1),
              "delete should succeed");
  expect_true("silence first word",
              toaster_transcript_silence_range(toaster_project_get_transcript(project), 0, 0),
              "silence should succeed");
  expect_true("add cut span",
              toaster_transcript_add_cut_span(toaster_project_get_transcript(project), 800000, 1000000),
              "cut span should append");
  expect_true("save project",
              toaster_project_save(project, path),
              "project should write to disk");

  loaded = toaster_project_load(path);
  expect_true("load project", loaded != NULL, "project should reload");
  expect_true("loaded media path",
              strcmp(toaster_project_get_media_path(loaded), "C:\\Media\\sample.mp4") == 0,
              "media path should round-trip");
  expect_true("loaded language",
              strcmp(toaster_project_get_language(loaded), "en-US") == 0,
              "language should round-trip");

  loaded_transcript = toaster_project_get_transcript_const(loaded);
  expect_true("loaded word count",
              toaster_transcript_word_count(loaded_transcript) == 2,
              "loaded transcript should keep words");
  expect_true("loaded word flags",
              toaster_transcript_get_word(loaded_transcript, 0, &word) && word.silenced &&
                toaster_transcript_get_word(loaded_transcript, 1, &word) && word.deleted,
              "loaded word flags should round-trip");
  expect_true("loaded cut span",
              toaster_transcript_cut_span_count(loaded_transcript) == 1 &&
                toaster_transcript_get_cut_span(loaded_transcript, 0, &range) &&
                range.start_us == 800000 && range.end_us == 1000000,
              "cut span should round-trip");
  expect_true("keep segment merge",
              toaster_transcript_keep_segment_count(loaded_transcript) == 1 &&
                toaster_transcript_get_keep_segment(loaded_transcript, 0, &range) &&
                range.start_us == 0 && range.end_us == 500000,
              "delete and cut span should collapse keep range");

  toaster_project_destroy(project);
  toaster_project_destroy(loaded);
  remove(path);
  toaster_shutdown();

  return failures ? 1 : 0;
}
