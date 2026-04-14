add_library(toaster SHARED
  toaster.c
  project.c
  analysis.c
  model.c
  transcribe.c
  callback/callback.c
)

target_compile_definitions(toaster PRIVATE toaster_EXPORTS)

target_include_directories(toaster
  PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
)

if(WIN32)
  target_link_libraries(toaster PRIVATE shell32 winhttp)
endif()

find_library(WHISPER_LIB whisper PATHS "${CMAKE_PREFIX_PATH}/lib" NO_DEFAULT_PATH)
if(NOT WHISPER_LIB)
  find_library(WHISPER_LIB whisper)
endif()

if(WHISPER_LIB)
  target_link_libraries(toaster PRIVATE ${WHISPER_LIB})
  target_compile_definitions(toaster PRIVATE TOASTER_HAS_WHISPER=1)
  message(STATUS "Found whisper: ${WHISPER_LIB}")
else()
  message(WARNING "whisper library not found; transcription will be disabled")
endif()
