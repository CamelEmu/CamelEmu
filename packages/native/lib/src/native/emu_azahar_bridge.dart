import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart';

final class _SessionConfig extends Struct {
  @Uint32()
  external int graphicsApi;

  @Uint32()
  external int cpuClockPercentage;

  @Uint32()
  external int numCores;
}

/// The values of `Core::System::ResultStatus`, which the Rust core returns as well.
enum EmuAzaharResultStatus {
  success,
  errorNotInitialized,
  errorGetLoader,
  errorSystemMode,
  errorLoader,
  errorLoaderErrorEncrypted,
  errorLoaderErrorInvalidFormat,
  errorLoaderErrorGbaTitle,
  errorSystemFiles,
  errorSavestate,
  errorArticDisconnected,
  errorArticCountryMismatch,
  shutdownRequested,
  errorUnknown;

  static EmuAzaharResultStatus fromRaw(int raw) =>
      raw >= 0 && raw < values.length ? values[raw] : errorUnknown;
}

/// Runs a game only in the Rust core. It calls `libemu_azahar.so` directly through `dart:ffi`,
/// and the screen textures come from the plugin, so nothing goes through the C++ core.
class EmuAzaharBridge {
  EmuAzaharBridge._(DynamicLibrary library)
    : _create = library
          .lookupFunction<
            Pointer<Void> Function(Pointer<_SessionConfig>),
            Pointer<Void> Function(Pointer<_SessionConfig>)
          >('emu_azahar_session_create'),
      _destroy = library
          .lookupFunction<
            Void Function(Pointer<Void>),
            void Function(Pointer<Void>)
          >('emu_azahar_session_destroy'),
      _load = library
          .lookupFunction<
            Uint32 Function(Pointer<Void>, Pointer<Utf8>),
            int Function(Pointer<Void>, Pointer<Utf8>)
          >('emu_azahar_session_load'),
      _start = library
          .lookupFunction<
            Uint32 Function(Pointer<Void>),
            int Function(Pointer<Void>)
          >('emu_azahar_session_start'),
      _pause = library
          .lookupFunction<
            Void Function(Pointer<Void>),
            void Function(Pointer<Void>)
          >('emu_azahar_session_pause'),
      _stop = library
          .lookupFunction<
            Void Function(Pointer<Void>),
            void Function(Pointer<Void>)
          >('emu_azahar_session_stop');

  static EmuAzaharBridge? _instance;

  /// Opens the library of the Rust core once.
  static EmuAzaharBridge open() =>
      _instance ??= EmuAzaharBridge._(DynamicLibrary.open('libemu_azahar.so'));

  final Pointer<Void> Function(Pointer<_SessionConfig>) _create;
  final void Function(Pointer<Void>) _destroy;
  final int Function(Pointer<Void>, Pointer<Utf8>) _load;
  final int Function(Pointer<Void>) _start;
  final void Function(Pointer<Void>) _pause;
  final void Function(Pointer<Void>) _stop;

  final MethodChannel _channel = const MethodChannel(
    'org.citra.citra_emu/azahar_bridge',
  );

  Pointer<Void> _session = nullptr;

  /// Creates the session. Returns false when the memory of the console could not be created.
  bool create({
    required int graphicsApi,
    required int cpuClockPercentage,
    required int numCores,
  }) {
    final config = calloc<_SessionConfig>();
    try {
      config.ref
        ..graphicsApi = graphicsApi
        ..cpuClockPercentage = cpuClockPercentage
        ..numCores = numCores;
      _session = _create(config);
    } finally {
      calloc.free(config);
    }
    return _session != nullptr;
  }

  EmuAzaharResultStatus load(String path) {
    final nativePath = path.toNativeUtf8();
    try {
      return EmuAzaharResultStatus.fromRaw(_load(_session, nativePath));
    } finally {
      calloc.free(nativePath);
    }
  }

  EmuAzaharResultStatus start() =>
      EmuAzaharResultStatus.fromRaw(_start(_session));

  void pause() => _pause(_session);

  void stop() => _stop(_session);

  void destroy() {
    if (_session == nullptr) return;
    _destroy(_session);
    _session = nullptr;
  }

  /// Creates a screen texture whose surface goes to the Rust core, and returns its id.
  Future<int> createTexture({
    required int width,
    required int height,
    required bool secondary,
  }) async {
    final textureId = await _channel.invokeMethod<int>(
      'createRustCoreTexture',
      {'width': width, 'height': height, 'secondary': secondary},
    );
    return textureId ?? -1;
  }

  Future<void> releaseTextures() =>
      _channel.invokeMethod<void>('releaseRustCoreTextures');
}
