import 'package:flutter/material.dart';

import '../../i18n/translations.g.dart';
import 'emulator_setting_key.dart';
import 'sections/rust_core_settings.dart';

/// A part of the emulation that can run in the Rust core instead of the C++ core, listed in the
/// order the feature flags page shows them.
enum RustCorePart {
  renderer(RustCoreSettingKeys.rustRenderer, Icons.brush_outlined),
  audio(RustCoreSettingKeys.rustAudio, Icons.volume_up_outlined),
  network(RustCoreSettingKeys.rustNetwork, Icons.wifi),
  kernel(RustCoreSettingKeys.rustKernel, Icons.memory),
  service(
    RustCoreSettingKeys.rustService,
    Icons.miscellaneous_services_outlined,
  );

  const RustCorePart(this.key, this.icon);

  final IntBoolKey key;
  final IconData icon;

  /// The other parts that have to run in Rust for this part to run in Rust.
  List<RustCorePart> get requires => switch (this) {
    service => const [kernel],
    _ => const [],
  };

  String title(Translations t) => switch (this) {
    renderer => t.settings.featureFlags.newEmulatorCore.parts.renderer.title,
    audio => t.settings.featureFlags.newEmulatorCore.parts.audio.title,
    network => t.settings.featureFlags.newEmulatorCore.parts.network.title,
    kernel => t.settings.featureFlags.newEmulatorCore.parts.kernel.title,
    service => t.settings.featureFlags.newEmulatorCore.parts.service.title,
  };

  String description(Translations t) => switch (this) {
    renderer =>
      t.settings.featureFlags.newEmulatorCore.parts.renderer.description,
    audio => t.settings.featureFlags.newEmulatorCore.parts.audio.description,
    network =>
      t.settings.featureFlags.newEmulatorCore.parts.network.description,
    kernel => t.settings.featureFlags.newEmulatorCore.parts.kernel.description,
    service =>
      t.settings.featureFlags.newEmulatorCore.parts.service.description,
  };
}
