import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../settings/feature_flags_provider.dart';
import '../../settings/rust_core_part.dart';
import '../../settings/sections/rust_core_settings.dart';
import '../abstract_base_option.dart';
import '../option_category.dart';
import '../option_dependency.dart';
import '../option_section.dart';
import '../option_value.dart';
import '../store_option_values.dart';

/// The items of the feature flags page, which is reached from the Other category: one switch for
/// each flag of [featureFlagsProvider], under the experimental heading. Switching one finds its
/// flag in that list and calls its `switchTo`.
///
/// The new emulator core heading follows, with the switch that uses the Rust core and one switch
/// for each [RustCorePart], which can only be switched while the Rust core and the parts it
/// requires are on.
final featureFlagsOptionsProvider = Provider<OptionCategory>((ref) {
  final featureFlags = ref.watch(featureFlagsProvider);
  return OptionCategory(
    id: 'featureFlags',
    title: (t) => t.settings.featureFlags.title,
    sections: [
      OptionSection(
        title: (t) => t.settings.featureFlags.experimental,
        options: [
          for (final featureFlag in featureFlags)
            BoolOption(
              title: featureFlag.title,
              description: featureFlag.description,
              icon: featureFlag.icon ?? Icons.flag_outlined,
              value: CallbackOptionValue<bool>(
                onRead: featureFlag.read,
                onWrite: (context, ref, value) => ref
                    .read(featureFlagsProvider)
                    .firstWhere((flag) => flag.id == featureFlag.id)
                    .switchTo(ref, value),
              ),
            ),
        ],
      ),
      OptionSection(
        title: (t) => t.settings.featureFlags.newEmulatorCore.title,
        options: [
          BoolOption(
            title: (t) =>
                t.settings.featureFlags.newEmulatorCore.useRustCore.title,
            description: (t) =>
                t.settings.featureFlags.newEmulatorCore.useRustCore.description,
            icon: Icons.science_outlined,
            value: const StoreBoolValue(RustCoreSettingKeys.rustCore),
          ),
          for (final part in RustCorePart.values)
            DependentBoolOption(
              title: part.title,
              description: part.description,
              icon: part.icon,
              value: StoreBoolValue(part.key),
              dependencies: [
                const BoolOptionDependency(
                  StoreBoolValue(RustCoreSettingKeys.rustCore),
                ),
                for (final required in part.requires)
                  BoolOptionDependency(StoreBoolValue(required.key)),
              ],
            ),
        ],
      ),
    ],
  );
});
