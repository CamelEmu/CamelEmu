// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#ifndef EMU_SESSION_H
#define EMU_SESSION_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque handle to one emulation session.
 *
 * Every resource the session creates (emulation thread, present thread,
 * windows, textures, GPU state, core system, input and network subsystems)
 * is owned by this handle and released by emu_session_destroy.
 */
typedef struct EmuSession EmuSession;

enum EmuShaderStage {
  EMU_SHADER_STAGE_PREPARE = 0,
  EMU_SHADER_STAGE_DECOMPILE = 1,
  EMU_SHADER_STAGE_BUILD = 2,
  EMU_SHADER_STAGE_COMPLETE = 3,
};

enum EmuStatus {
  EMU_STATUS_OK = 0,
  EMU_STATUS_INVALID_ARGUMENT = 1,
  EMU_STATUS_LOAD_FAILED = 2,
  EMU_STATUS_NOT_RUNNING = 3,
  EMU_STATUS_INVALID_ADDRESS = 4,
};

/**
 * Callbacks invoked by the session from its own threads.
 *
 * The callbacks and `user` stay valid until emu_session_destroy returns.
 * After that call no callback is invoked any more.
 */
typedef struct EmuSessionCallbacks {
  void* user;
  void (*on_shader_progress)(void* user, int32_t stage, uint64_t progress, uint64_t max);
  void (*on_texture)(void* user, int64_t texture_id, int32_t secondary);
  void (*on_error)(void* user, const char* message);
  /** Pushes stereo s16 frames at EMU_AUDIO_SAMPLE_RATE; when NULL the core plays through OpenAL. */
  void (*on_audio)(void* user, const int16_t* frames, size_t frame_count);
  /**
   * Called once when the emulated program asks to shut down, after its emulation loop ended.
   * The session still has to be destroyed; the application decides how.
   */
  void (*on_shutdown_requested)(void* user);
  void (*on_artic_country_mismatch)(void* user, uint32_t region, uint32_t country);
  /**
   * Delivers a requested screenshot as `width` x `height` BGRA pixels, bottom row first.
   * `pixels` is NULL when the screenshot failed, and valid only during the call.
   */
  void (*on_screenshot)(void* user, uint64_t request_id, int32_t secondary, int32_t width,
                        int32_t height, const uint8_t* pixels);
  /** Delivers the serialized draw data of one frame. `data` is valid only during the call. */
  void (*on_capture_3d)(void* user, uint64_t request_id, const uint8_t* data, size_t length);
} EmuSessionCallbacks;

#define EMU_AUDIO_SAMPLE_RATE 32728
#define EMU_AUDIO_CHANNELS 2

/** Size of the screen textures and whether a separate bottom screen texture is wanted. */
typedef struct EmuSessionOptions {
  int32_t primary_width;
  int32_t primary_height;
  int32_t secondary_width;
  int32_t secondary_height;
  int32_t dual_screen;
} EmuSessionOptions;

/** Creates a session for the given game path. Returns NULL on failure. */
EmuSession* emu_session_create(const char* game_path, const EmuSessionOptions* options,
                                     const EmuSessionCallbacks* callbacks);

/** Starts the emulation and shader preparation threads. */
int32_t emu_session_start(EmuSession* session);

void emu_session_answer_artic_country(EmuSession* session, int32_t country);

int32_t emu_session_pause(EmuSession* session);
int32_t emu_session_resume(EmuSession* session);

/** Returns the size in bytes of the FCRAM of the loaded system, or 0 when not running. */
size_t emu_session_fcram_size(const EmuSession* session);

/**
 * Copies `len` bytes starting at the 3DS virtual address `address` into `out`.
 * Returns EMU_STATUS_INVALID_ADDRESS when any part of the range is unmapped.
 */
int32_t emu_session_read_memory(EmuSession* session, uint32_t address, uint8_t* out, size_t len);

/**
 * Copies `len` bytes of FCRAM starting at `offset` into `out`.
 * Returns EMU_STATUS_INVALID_ADDRESS when the range exceeds the FCRAM size.
 */
int32_t emu_session_read_fcram(EmuSession* session, size_t offset, uint8_t* out, size_t len);

/**
 * Requests a screenshot of the top (`secondary` == 0) or bottom screen. The result arrives through
 * on_screenshot with the same `request_id`.
 * Returns EMU_STATUS_NOT_RUNNING when no game is running or a screenshot is already pending.
 */
int32_t emu_session_request_screenshot(EmuSession* session, uint64_t request_id,
                                       int32_t secondary);

/**
 * Requests the draw data of the next complete frame. The result arrives through on_capture_3d.
 * Returns EMU_STATUS_NOT_RUNNING when no game is running or a capture is already pending.
 */
int32_t emu_session_request_capture_3d(EmuSession* session, uint64_t request_id);

/**
 * Stops the session, joins every thread it owns and frees every resource,
 * including the emulated 3DS memory. Blocks until all of that is complete.
 */
void emu_session_destroy(EmuSession* session);

#ifdef __cplusplus
}
#endif

#endif
