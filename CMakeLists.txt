cmake_minimum_required(VERSION 3.20)

if(NOT CMAKE_TOOLCHAIN_FILE)
  set(CMAKE_TOOLCHAIN_FILE "${CMAKE_CURRENT_SOURCE_DIR}/toolchain.cmake"
      CACHE FILEPATH "CMake toolchain file")
endif()

if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(CMAKE_INSTALL_PREFIX "/system" CACHE PATH "Install prefix" FORCE)
endif()

project(cps VERSION 0.0.70 LANGUAGES NONE)

include(GNUInstallDirs)

find_program(CARGO_EXECUTABLE cargo REQUIRED)

if(CMAKE_SYSTEM_PROCESSOR STREQUAL "x86_64")
  set(RUST_TARGET x86_64-unknown-linux-musl)
elseif(CMAKE_SYSTEM_PROCESSOR STREQUAL "aarch64")
  set(RUST_TARGET aarch64-unknown-linux-musl)
else()
  message(FATAL_ERROR
    "Unsupported architecture: ${CMAKE_SYSTEM_PROCESSOR}. Supported: x86_64, aarch64")
endif()

set(CPS_CARGO_DIR "${CMAKE_BINARY_DIR}/cargo")
set(CPS_OUTPUT    "${CMAKE_BINARY_DIR}/cps")

add_custom_command(
  OUTPUT "${CPS_OUTPUT}"
  COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${CPS_CARGO_DIR}
          ${CARGO_EXECUTABLE} build --release --locked --bin cps
          --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
          --target ${RUST_TARGET}
  COMMAND ${CMAKE_COMMAND} -E copy
          "${CPS_CARGO_DIR}/${RUST_TARGET}/release/cps" "${CPS_OUTPUT}"
  DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  USES_TERMINAL
  VERBATIM
)

add_custom_target(cps_bin ALL DEPENDS "${CPS_OUTPUT}")

install(PROGRAMS "${CPS_OUTPUT}" DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES themes/cps.py themes/minimal.py
        DESTINATION ${CMAKE_INSTALL_DATADIR}/cps/themes)
install(FILES t.desc p.desc cps.toml
        DESTINATION ${CMAKE_INSTALL_DATADIR}/cps)
install(FILES docs/cps.1 DESTINATION ${CMAKE_INSTALL_MANDIR}/man1)
