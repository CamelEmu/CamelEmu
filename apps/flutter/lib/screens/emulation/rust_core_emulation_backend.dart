import 'dart:ui';

import 'package:azahar_for_flutter/azahar_for_flutter.dart';

import '../../app_services.dart';
import '../../data/settings/sections/debug_settings.dart';
import '../../data/settings/sections/graphics_settings.dart';
import '../../data/settings/sections/system_settings.dart';
import 'emulation_backend.dart';

/// Runs one game only in the Rust core, through [EmuAzaharBridge], without the C++ core.
class RustCoreEmulationBackend implements EmulationBackend {
  final EmuAzaharBridge _bridge = EmuAzaharBridge.open();
  bool _started = false;

  @override
  Future<void> start({
    required String gamePath,
    required Size topScreenSize,
    required Size bottomScreenSize,
    required AudioEngine audioEngine,
    required EmulationBackendListener listener,
  }) async {
    final settings = AppServices.emulatorSettingsRepository;
    final created = _bridge.create(
      graphicsApi: settings.readInt(GraphicsSettingKeys.graphicsApi),
      cpuClockPercentage: settings.readInt(DebugSettingKeys.cpuClockSpeed),
      numCores: settings.readBool(SystemSettingKeys.new3ds) ? 4 : 2,
    );
    if (!created) {
      listener.onError(
        'The emulation ended with status '
        '${EmuAzaharResultStatus.errorNotInitialized.index}',
      );
      return;
    }
    _started = true;

    listener.onTexture(
      await _bridge.createTexture(
        width: topScreenSize.width.round(),
        height: topScreenSize.height.round(),
        secondary: false,
      ),
      secondary: false,
    );
    listener.onTexture(
      await _bridge.createTexture(
        width: bottomScreenSize.width.round(),
        height: bottomScreenSize.height.round(),
        secondary: true,
      ),
      secondary: true,
    );

    var status = _bridge.load(gamePath);
    if (status == EmuAzaharResultStatus.success) {
      status = _bridge.start();
    }
    if (status != EmuAzaharResultStatus.success) {
      listener.onError('The emulation ended with status ${status.index}');
      return;
    }
    listener.onShaderProgress(
      const ShaderCacheProgress(
        stage: ShaderCacheStage.complete,
        progress: 0,
        max: 0,
      ),
    );
  }

  @override
  Future<void> answerCountry(int? country) async {}

  @override
  Future<void> pause() async => _bridge.pause();

  @override
  Future<void> resume() async => _bridge.start();

  @override
  Future<void> stop({required bool persist}) async {
    if (!_started) return;
    _started = false;
    _bridge.stop();
    _bridge.destroy();
    await _bridge.releaseTextures();
  }
}
