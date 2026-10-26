set(EMU_DIR "${AZAHAR_REPO_ROOT}/packages/emu")

# The same sources are built into two libraries. `citra-android` is the one `System.loadLibrary`
# loads and the application never lets go of. `azahar-session` is loaded and unloaded by the Rust
# crate for every game, so every global of the emulator is gone when the session is dropped.
function(azahar_android_library target)
    add_library(${target} SHARED
        android_common/android_common.cpp
        android_common/android_common.h
        applets/mii_selector.cpp
        applets/mii_selector.h
        applets/swkbd.cpp
        applets/swkbd.h
        input_manager.cpp
        input_manager.h
        camera/ndk_camera.cpp
        camera/ndk_camera.h
        camera/still_image_camera.cpp
        camera/still_image_camera.h
        cheats/cheat.cpp
        cheats/cheat.h
        cheats/cheat_engine.cpp
        cheats/memory_tools.cpp
        config.cpp
        config.h
        default_ini.h
        emu_window/emu_window.cpp
        emu_window/emu_window.h
        id_cache.cpp
        id_cache.h
        native.cpp
        ndk_motion.cpp
        ndk_motion.h
        system_save_game.cpp
        native_log.cpp
        util.cpp
        util.h
    )

    target_include_directories(${target} PRIVATE "${AZAHAR_REPO_ROOT}/packages/native/include")
    target_link_options(${target} PRIVATE -Wl,-z,noexecstack)
    target_link_libraries(${target} PRIVATE audio_core citra_common citra_core input_common network)
    target_link_libraries(${target} PRIVATE android camera2ndk inih jnigraphics log mediandk)

    if (ENABLE_OPENGL)
        target_sources(${target} PRIVATE
            emu_window/emu_window_gl.cpp
            emu_window/emu_window_gl.h
        )
        target_link_libraries(${target} PRIVATE EGL glad)
    endif()
    if (ENABLE_VULKAN)
        target_sources(${target} PRIVATE
            emu_window/emu_window_vk.cpp
            emu_window/emu_window_vk.h
        )
        if ("arm64" IN_LIST ARCHITECTURE)
            target_link_libraries(${target} PRIVATE adrenotools)
        endif()
    endif()
endfunction()

azahar_android_library(citra-android)
azahar_android_library(azahar-session)

if ("${ANDROID_ABI}" STREQUAL "arm64-v8a")
    set(EMU_TARGET aarch64-linux-android)
elseif ("${ANDROID_ABI}" STREQUAL "x86_64")
    set(EMU_TARGET x86_64-linux-android)
else()
    message(FATAL_ERROR "emu does not support the ABI ${ANDROID_ABI}")
endif()
string(REPLACE "-" "_" EMU_TARGET_ENV "${EMU_TARGET}")
string(TOUPPER "${EMU_TARGET_ENV}" EMU_TARGET_UPPER)
set(EMU_CLANG "${ANDROID_TOOLCHAIN_ROOT}/bin/${EMU_TARGET}${ANDROID_PLATFORM_LEVEL}-clang")
set(EMU_LIB "${EMU_DIR}/target/${EMU_TARGET}/release/libemu.a")
find_program(CARGO_EXECUTABLE cargo REQUIRED)
set(LIBRESSL_STUB_DIR "${CMAKE_CURRENT_BINARY_DIR}/libressl-stub")
add_custom_target(emu_lib
    COMMAND ${CMAKE_COMMAND} -E make_directory "${LIBRESSL_STUB_DIR}"
    COMMAND "${CMAKE_AR}" qc "${LIBRESSL_STUB_DIR}/libssl.a"
    COMMAND "${CMAKE_AR}" qc "${LIBRESSL_STUB_DIR}/libcrypto.a"
    COMMAND ${CMAKE_COMMAND} -E env
        "CC_${EMU_TARGET_ENV}=${EMU_CLANG}"
        "CXX_${EMU_TARGET_ENV}=${EMU_CLANG}++"
        "AR_${EMU_TARGET_ENV}=${ANDROID_TOOLCHAIN_ROOT}/bin/llvm-ar"
        "CARGO_TARGET_${EMU_TARGET_UPPER}_LINKER=${EMU_CLANG}"
        "OPENSSL_INCLUDE_DIR=${EMU_DIR}/crates/http/libressl/include"
        "OPENSSL_LIB_DIR=${LIBRESSL_STUB_DIR}"
        OPENSSL_STATIC=1
        "${CARGO_EXECUTABLE}" rustc -p emu --release --lib --crate-type staticlib
        --target ${EMU_TARGET}
    WORKING_DIRECTORY "${EMU_DIR}"
    BYPRODUCTS "${EMU_LIB}"
    COMMENT "Building emu for ${EMU_TARGET}"
)

# Only the library of the application contains the crate. It loads the library of the session.
add_dependencies(citra-android emu_lib)
target_sources(citra-android PRIVATE host_storage.cpp)
target_link_libraries(citra-android PRIVATE
    -Wl,--whole-archive
    "${EMU_LIB}"
    -Wl,--no-whole-archive
    ssl
    crypto
    OpenSLES
)
target_link_options(citra-android PRIVATE "-Wl,--export-dynamic-symbol=frb_*")

foreach (target azahar-session tests)
    if (TARGET ${target})
        add_dependencies(${target} emu_lib)
        target_link_libraries(${target} PRIVATE "${EMU_LIB}" ssl crypto)
    endif()
endforeach()

set(CPACK_PACKAGE_EXECUTABLES ${CPACK_PACKAGE_EXECUTABLES} citra-android azahar-session)
