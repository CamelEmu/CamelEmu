import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../screenshot/screenshot_settings_provider.dart';
import '../abstract_base_option.dart';
import '../option_category.dart';
import '../option_section.dart';
import '../option_value.dart';

final screenshotOptionsProvider = Provider<OptionCategory>(
  (ref) => OptionCategory(
    id: 'screenshot',
    title: (t) => t.options.screenshotSettings,
    sections: [
      OptionSection(
        options: [
          ActionOption(
            title: (t) => t.options.screenshotDirectory,
            description: (t) => t.options.screenshotDirectoryDescription,
            icon: Icons.folder_outlined,
            onTap: (context, ref) =>
                ref.read(screenshotSettingsProvider.notifier).chooseDirectory(),
          ),
          BoolOption(
            title: (t) => t.options.screenshotAlwaysAsk,
            description: (t) => t.options.screenshotAlwaysAskDescription,
            icon: Icons.help_outline,
            value: CallbackOptionValue<bool>(
              onRead: (ref) =>
                  ref.watch(screenshotSettingsProvider).value?.alwaysAsk ??
                  false,
              onWrite: (context, ref, value) => ref
                  .read(screenshotSettingsProvider.notifier)
                  .setAlwaysAsk(value),
            ),
          ),
        ],
      ),
    ],
  ),
);
