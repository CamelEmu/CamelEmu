import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app_services.dart';
import '../options/option_values_revision_provider.dart';
import 'rust_core_part.dart';
import 'rust_cpu_provider.dart';
import 'sections/rust_core_settings.dart';

/// How much of the emulation runs in the Rust core.
enum RustCoreMode {
  /// Everything runs in the C++ core.
  off,

  /// The C++ core runs, with the parts that are turned on running in the Rust core.
  partial,

  /// Every part runs in the Rust core, without the C++ core.
  full,
}

/// The [RustCoreMode] of the current settings. It is [RustCoreMode.off] on builds without the Rust
/// core.
final rustCoreModeProvider = Provider<RustCoreMode>((ref) {
  ref.watch(optionValuesRevisionProvider);
  if (!(ref.watch(rustCpuSupportedProvider).value ?? false)) {
    return RustCoreMode.off;
  }
  final settings = AppServices.emulatorSettingsRepository;
  if (!settings.readBool(RustCoreSettingKeys.rustCore)) {
    return RustCoreMode.off;
  }
  final everyPart = RustCorePart.values.every(
    (part) => settings.readBool(part.key),
  );
  return everyPart ? RustCoreMode.full : RustCoreMode.partial;
});
