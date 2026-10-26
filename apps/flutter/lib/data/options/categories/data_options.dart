import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../routing/app_routes.dart';
import '../abstract_base_option.dart';
import '../option_category.dart';
import '../option_section.dart';

final dataOptionsProvider = Provider<OptionCategory>(
  (ref) => OptionCategory(
    id: 'data',
    title: (t) => t.options.groups.data,
    sections: [
      OptionSection(
        options: [
          NestedOption(
            title: (t) => t.options.screenshotSettings,
            description: (t) => t.options.screenshotSettingsDescription,
            icon: Icons.screenshot_outlined,
            destination: const OptionsScreenshotSettingsRoute(),
          ),
        ],
      ),
    ],
  ),
);
