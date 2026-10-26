// Copyright 2016 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <emu_audio.h>

#include "audio_core/time_stretch.h"
#include "common/assert.h"
#include "common/logging/log.h"

namespace AudioCore {

void TimeStretcher::Deleter::operator()(EmuTimeStretcher* handle) const {
    emu_time_stretcher_free(handle);
}

TimeStretcher::TimeStretcher() : handle(emu_time_stretcher_new()) {
    ASSERT_MSG(handle != nullptr, "Could not create the time stretcher");
}

TimeStretcher::~TimeStretcher() = default;

void TimeStretcher::SetOutputSampleRate(unsigned int sample_rate) {
    if (emu_time_stretcher_set_output_sample_rate(handle.get(), sample_rate) != 0) {
        LOG_ERROR(Audio, "Could not set the output sample rate {} of the time stretcher",
                  sample_rate);
    }
}

std::size_t TimeStretcher::Process(const s16* in, std::size_t num_in, s16* out,
                                   std::size_t num_out) {
    return emu_time_stretcher_process(handle.get(), in, num_in, out, num_out);
}

void TimeStretcher::Clear() {
    emu_time_stretcher_clear(handle.get());
}

void TimeStretcher::Flush() {
    if (emu_time_stretcher_flush(handle.get()) != 0) {
        LOG_ERROR(Audio, "Could not flush the time stretcher");
    }
}

} // namespace AudioCore
