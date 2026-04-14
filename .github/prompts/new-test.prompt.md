---
description: "Scaffold a new Toaster test executable with PASS/FAIL macros, startup/shutdown, and CMake wiring."
argument-hint: "test-name (e.g. test-project)"
agent: "agent"
---
Create a new Toaster test executable with the given name. Generate:

1. `test/{name}.c` with:
   - `#include "toaster.h"` and standard headers
   - Static `failures` counter
   - PASS/FAIL macros:
     ```c
     static int failures = 0;
     #define PASS(name) printf("  PASS: %s\n", name)
     #define FAIL(name, msg) do { printf("  FAIL: %s — %s\n", name, msg); failures++; } while (0)
     ```
   - Stub test function(s) relevant to the test name
   - `main()` that calls `toaster_startup()`, runs tests, calls `toaster_shutdown()`, returns `failures ? 1 : 0`
2. Add the executable to `test/CMakeLists.txt` following the existing pattern (link to `toaster` library, set include dirs)

Follow all conventions in [.github/copilot-instructions.md](../copilot-instructions.md).
