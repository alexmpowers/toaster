---
description: "Use when creating, editing, or reviewing plugin code — filters, decoders, exporters, encoders. Covers the plugin registration pattern, info struct layout, and CMake wiring."
applyTo: "plugins/**"
---
# Plugin Authoring

Every plugin follows create → operate → destroy with a static info struct:

1. **Include only `toaster.h`** — never include Qt or frontend headers
2. **Define a static `toaster_{type}_info_t`** with `.id`, `.get_name`, `.create`, `.destroy`, and type-specific callbacks
3. **Export `{plugin_name}_load(void)`** that calls `toaster_register_{type}(&info)`
4. **Call the load function after `toaster_startup()`** — never use `__attribute__((constructor))`

## Naming

- File: `{name}.c` (one source file per plugin unless complex)
- CMake target: `toaster-{name}`
- Info struct: `{name}_info` (file-static)
- Load function global: `{name}_load(void)`

## CMakeLists.txt pattern

```cmake
add_library(toaster-{name} SHARED {name}.c)
target_include_directories(toaster-{name} PRIVATE ${CMAKE_SOURCE_DIR}/libtoaster)
target_link_libraries(toaster-{name} PRIVATE toaster)
```

Register the new target in `plugins/CMakeLists.txt` via `add_subdirectory({name})`.

## Canonical example

See `plugins/filler-filter/filler-filter.c` for the simplest complete plugin.
