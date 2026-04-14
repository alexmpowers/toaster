#include <stdio.h>

#include "toaster.h"

static int failures = 0;

#define PASS(name) printf("  PASS: %s\n", name)
#define FAIL(name, msg)                                                              \
  do {                                                                               \
    printf("  FAIL: %s - %s\n", name, msg);                                          \
    failures++;                                                                      \
  } while (0)

typedef struct signal_probe {
  int count;
  int last_value;
} signal_probe_t;

static void probe_callback(const char *signal, void *param, void *user_data)
{
  int *value = (int *)param;
  signal_probe_t *probe = (signal_probe_t *)user_data;

  if (signal && probe) {
    probe->count += 1;
    probe->last_value = value ? *value : -1;
  }
}

static void expect_true(const char *name, bool condition, const char *message)
{
  if (condition)
    PASS(name);
  else
    FAIL(name, message);
}

int main(void)
{
  toaster_signal_handler_t *handler;
  signal_probe_t probe = {0};
  int value = 7;

  toaster_startup();

  handler = toaster_signal_handler_create();
  expect_true("create handler", handler != NULL, "signal handler should allocate");

  expect_true("connect callback",
              toaster_signal_handler_connect(handler, "changed", probe_callback, &probe),
              "connect should succeed");
  toaster_signal_handler_emit(handler, "changed", &value);
  expect_true("callback fired once",
              probe.count == 1 && probe.last_value == 7,
              "emit should forward payload to callback");

  value = 13;
  toaster_signal_handler_emit(handler, "other-signal", &value);
  expect_true("other signal ignored",
              probe.count == 1,
              "different signal name should not call callback");

  expect_true("disconnect callback",
              toaster_signal_handler_disconnect(handler, "changed", probe_callback, &probe),
              "disconnect should remove callback");
  toaster_signal_handler_emit(handler, "changed", &value);
  expect_true("callback stays disconnected",
              probe.count == 1,
              "disconnected callback should not fire");

  toaster_signal_handler_destroy(handler);
  toaster_shutdown();

  return failures ? 1 : 0;
}
