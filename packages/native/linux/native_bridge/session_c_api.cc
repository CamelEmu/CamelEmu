#include "emu_session.h"

#include <memory>
#include <string>

#include "emulation.h"

struct EmuSession {
  EmuSessionCallbacks callbacks;
  std::unique_ptr<Emulation::Session> session;
};

namespace {

Emulation::SessionCallbacks MakeCallbacks(const EmuSessionCallbacks& raw) {
  Emulation::SessionCallbacks callbacks;
  if (raw.on_shader_progress != nullptr) {
    callbacks.on_shader_progress = [raw](int32_t stage, uint64_t progress, uint64_t max) {
      raw.on_shader_progress(raw.user, stage, progress, max);
    };
  }
  if (raw.on_texture != nullptr) {
    callbacks.on_texture = [raw](int64_t texture_id, bool secondary) {
      raw.on_texture(raw.user, texture_id, secondary ? 1 : 0);
    };
  }
  if (raw.on_error != nullptr) {
    callbacks.on_error = [raw](const std::string& message) {
      raw.on_error(raw.user, message.c_str());
    };
  }
  if (raw.on_audio != nullptr) {
    callbacks.on_audio = [raw](const int16_t* frames, std::size_t frame_count) {
      raw.on_audio(raw.user, frames, frame_count);
    };
  }
  if (raw.on_shutdown_requested != nullptr) {
    callbacks.on_shutdown_requested = [raw] { raw.on_shutdown_requested(raw.user); };
  }
  return callbacks;
}

}  // namespace

extern "C" {

EmuSession* emu_session_create(const char* game_path, const EmuSessionOptions* options,
                                     const EmuSessionCallbacks* callbacks) {
  if (game_path == nullptr || options == nullptr || callbacks == nullptr) {
    return nullptr;
  }
  auto wrapper = std::make_unique<EmuSession>();
  wrapper->callbacks = *callbacks;

  Emulation::SessionOptions session_options;
  session_options.primary_width = options->primary_width;
  session_options.primary_height = options->primary_height;
  session_options.secondary_width = options->secondary_width;
  session_options.secondary_height = options->secondary_height;
  session_options.dual_screen = options->dual_screen != 0;

  wrapper->session = std::make_unique<Emulation::Session>(
      game_path, session_options, MakeCallbacks(wrapper->callbacks));
  return wrapper.release();
}

int32_t emu_session_start(EmuSession* session) {
  if (session == nullptr) {
    return EMU_STATUS_INVALID_ARGUMENT;
  }
  return session->session->Start() ? EMU_STATUS_OK : EMU_STATUS_LOAD_FAILED;
}

int32_t emu_session_pause(EmuSession* session) {
  if (session == nullptr) {
    return EMU_STATUS_INVALID_ARGUMENT;
  }
  session->session->Pause();
  return EMU_STATUS_OK;
}

int32_t emu_session_resume(EmuSession* session) {
  if (session == nullptr) {
    return EMU_STATUS_INVALID_ARGUMENT;
  }
  session->session->Resume();
  return EMU_STATUS_OK;
}

size_t emu_session_fcram_size(const EmuSession* session) {
  return session == nullptr ? 0 : session->session->FcramSize();
}

int32_t emu_session_read_memory(EmuSession* session, uint32_t address, uint8_t* out,
                                   size_t len) {
  if (session == nullptr || out == nullptr) {
    return EMU_STATUS_INVALID_ARGUMENT;
  }
  return session->session->ReadMemory(address, out, len) ? EMU_STATUS_OK
                                                         : EMU_STATUS_INVALID_ADDRESS;
}

int32_t emu_session_read_fcram(EmuSession* session, size_t offset, uint8_t* out, size_t len) {
  if (session == nullptr || out == nullptr) {
    return EMU_STATUS_INVALID_ARGUMENT;
  }
  return session->session->ReadFcram(offset, out, len) ? EMU_STATUS_OK
                                                       : EMU_STATUS_INVALID_ADDRESS;
}

// TODO: Linux does not raise the Artic country mismatch callback yet, so there is no pending
// question to answer. Implement this together with the callback.
void emu_session_answer_artic_country(EmuSession*, int32_t) {}

void emu_session_destroy(EmuSession* session) {
  delete session;
}

}
