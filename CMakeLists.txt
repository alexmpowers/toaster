cmake_minimum_required(VERSION 3.22)

project(Toaster VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

option(TOASTER_BUILD_FRONTEND "Build the Qt frontend" ON)
option(TOASTER_BUILD_TESTS "Build standalone tests" ON)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)

add_subdirectory(libtoaster)

if(TOASTER_BUILD_FRONTEND)
  add_subdirectory(frontend)
endif()

if(TOASTER_BUILD_TESTS)
  enable_testing()
  add_subdirectory(test)
endif()
