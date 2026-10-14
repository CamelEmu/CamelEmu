import '../emulator_setting_key.dart';

abstract final class RustCoreSettingKeys {
  static const rustCore = IntBoolKey('Core', 'use_rust_core', false);
  static const rustCpu = IntBoolKey('Core', 'use_rust_cpu', false);
  static const rustRenderer = IntBoolKey('Core', 'rust_renderer', false);
  static const rustAudio = IntBoolKey('Core', 'rust_audio', false);
  static const rustNetwork = IntBoolKey('Core', 'rust_network', false);
  static const rustKernel = IntBoolKey('Core', 'rust_kernel', false);
  static const rustService = IntBoolKey('Core', 'rust_service', false);
}
