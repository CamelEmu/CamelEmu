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

uint32_t emu_azahar_version(void);

/** Returns null when `config` is null. */
EmuAzahar* emu_azahar_create(const EmuAzaharConfig* config);
void emu_azahar_destroy(EmuAzahar* core);

#ifdef __cplusplus
}
#endif

#endif
