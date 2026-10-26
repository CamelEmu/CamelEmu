#ifndef EMU_AUDIO_H
#define EMU_AUDIO_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct EmuTimeStretcher EmuTimeStretcher;

EmuTimeStretcher* emu_time_stretcher_new(void);

void emu_time_stretcher_free(EmuTimeStretcher* handle);

int32_t emu_time_stretcher_set_output_sample_rate(EmuTimeStretcher* handle, uint32_t sample_rate);

size_t emu_time_stretcher_process(EmuTimeStretcher* handle, const int16_t* in, size_t num_in,
                                  int16_t* out, size_t num_out);

void emu_time_stretcher_clear(EmuTimeStretcher* handle);

int32_t emu_time_stretcher_flush(EmuTimeStretcher* handle);

#ifdef __cplusplus
}
#endif

#endif
