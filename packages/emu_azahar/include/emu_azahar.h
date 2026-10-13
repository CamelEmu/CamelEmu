#ifndef EMU_AZAHAR_H
#define EMU_AZAHAR_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** The Rust core, which the crate allocates and owns. */
typedef struct EmuAzahar EmuAzahar;

/** Which parts run in the Rust core while the C++ core runs the rest. */
typedef struct EmuAzaharConfig {
  bool renderer;
  bool audio;
  bool network;
  bool kernel;
  bool service;
} EmuAzaharConfig;

/** A game that runs only in the Rust core, without the C++ core. */
typedef struct EmuAzaharSession EmuAzaharSession;

typedef struct EmuAzaharSessionConfig {
  /** The values of `Settings::GraphicsAPI`. */
  uint32_t graphics_api;
  uint32_t cpu_clock_percentage;
  uint32_t num_cores;
} EmuAzaharSessionConfig;

uint32_t emu_azahar_version(void);

/** Returns null when `config` is null. */
EmuAzahar* emu_azahar_create(const EmuAzaharConfig* config);
void emu_azahar_destroy(EmuAzahar* core);

/** Returns null when `config` is null or the memory could not be created. */
EmuAzaharSession* emu_azahar_session_create(const EmuAzaharSessionConfig* config);
void emu_azahar_session_destroy(EmuAzaharSession* session);
/** These return the values of `Core::System::ResultStatus`. */
uint32_t emu_azahar_session_load(EmuAzaharSession* session, const char* path);
uint32_t emu_azahar_session_start(EmuAzaharSession* session);
void emu_azahar_session_pause(EmuAzaharSession* session);
void emu_azahar_session_stop(EmuAzaharSession* session);

#ifdef __cplusplus
}
#endif

#endif
