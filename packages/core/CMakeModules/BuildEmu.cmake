# The TLS client of the emu_http crate links the LibreSSL whose handshake the servers of the console
# accept, so the library is built here for the crate instead of being bundled with the core.
set(EMU_DIR "${AZAHAR_REPO_ROOT}/packages/emu")
set(EMU_LIBRESSL_DIR "${EMU_DIR}/crates/http/libressl")

set(LIBRESSL_SKIP_INSTALL ON CACHE BOOL "")
set(OPENSSLDIR "/etc/ssl/")
add_subdirectory("${EMU_LIBRESSL_DIR}" "${PROJECT_BINARY_DIR}/externals/libressl" EXCLUDE_FROM_ALL)
target_include_directories(ssl SYSTEM INTERFACE "${EMU_LIBRESSL_DIR}/include")
target_compile_definitions(ssl PRIVATE -DHAVE_INET_NTOP)

# packages/emu/.cargo/config.toml points a plain cargo build at these links.
file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/libressl")
file(CREATE_LINK "${PROJECT_BINARY_DIR}/externals/libressl/ssl/libssl.a"
     "${CMAKE_BINARY_DIR}/libressl/libssl.a" SYMBOLIC)
file(CREATE_LINK "${PROJECT_BINARY_DIR}/externals/libressl/crypto/libcrypto.a"
     "${CMAKE_BINARY_DIR}/libressl/libcrypto.a" SYMBOLIC)

# The frontends of the platforms that do not go through the plugins link the crate directly. The
# Android library builds it for its own ABI.
if (ENABLE_WEB_SERVICE AND NOT ANDROID)
    find_program(CARGO_EXECUTABLE cargo REQUIRED)
    find_package(Threads REQUIRED)
    set(EMU_LIB "${EMU_DIR}/target/release/libemu.a")
    set(EMU_LIBRESSL_STUB_DIR "${PROJECT_BINARY_DIR}/libressl-stub")
    add_custom_target(emu_crate
        COMMAND "${CMAKE_COMMAND}" -E make_directory "${EMU_LIBRESSL_STUB_DIR}"
        COMMAND "${CMAKE_AR}" qc "${EMU_LIBRESSL_STUB_DIR}/libssl.a"
        COMMAND "${CMAKE_AR}" qc "${EMU_LIBRESSL_STUB_DIR}/libcrypto.a"
        COMMAND "${CMAKE_COMMAND}" -E env
            "OPENSSL_INCLUDE_DIR=${EMU_LIBRESSL_DIR}/include"
            "OPENSSL_LIB_DIR=${EMU_LIBRESSL_STUB_DIR}"
            OPENSSL_STATIC=1
            "${CARGO_EXECUTABLE}" rustc -p emu --release --lib --crate-type staticlib
        WORKING_DIRECTORY "${EMU_DIR}"
        BYPRODUCTS "${EMU_LIB}"
        COMMENT "Building emu (${EMU_DIR})"
    )

    add_library(emu STATIC IMPORTED GLOBAL)
    set_target_properties(emu PROPERTIES
        IMPORTED_LOCATION "${EMU_LIB}"
        INTERFACE_INCLUDE_DIRECTORIES "${AZAHAR_REPO_ROOT}/packages/native/include")
    add_dependencies(emu emu_crate)
    target_link_libraries(emu INTERFACE ssl crypto Threads::Threads ${CMAKE_DL_LIBS} m)
endif()
