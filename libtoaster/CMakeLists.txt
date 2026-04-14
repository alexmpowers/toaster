add_library(toaster SHARED
  toaster.c
  project.c
  analysis.c
  callback/callback.c
)

target_compile_definitions(toaster PRIVATE toaster_EXPORTS)

target_include_directories(toaster
  PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
)
