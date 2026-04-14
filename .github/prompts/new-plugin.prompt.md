---
description: "Scaffold a new Toaster plugin (filter, decoder, exporter, or encoder) with source file, CMakeLists.txt, and registration wiring."
argument-hint: "plugin-name type (filter|decoder|exporter|encoder)"
agent: "agent"
---
Create a new Toaster plugin with the given name and type. Generate:

1. `plugins/{name}/CMakeLists.txt` following the pattern in `plugins/filler-filter/CMakeLists.txt`
2. `plugins/{name}/{name}.c` with:
   - Header comment describing the plugin
   - `#include "toaster.h"` and necessary standard headers
   - Static `toaster_{type}_info_t` struct with `.id`, `.get_name`, `.create`, `.destroy`, and type-specific callbacks
   - A `{name}_load(void)` function that calls `toaster_register_{type}(&info)`
   - Stub implementations for all callbacks
3. Add `add_subdirectory({name})` to `plugins/CMakeLists.txt`
4. Add the `{name}_load()` call in the appropriate startup location

Follow all conventions in [.github/copilot-instructions.md](../copilot-instructions.md): `toaster_` prefix, `snake_case`, `calloc` for allocation, `bool` returns, timestamps in microseconds.
