import 'package:flutter/material.dart';

import '../../../data/options/categories/screenshot_options.dart';
import '../../options/option_category_page.dart';

class ScreenshotSettingsPage extends StatelessWidget {
  const ScreenshotSettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    return OptionCategoryPage(provider: screenshotOptionsProvider);
  }
}
