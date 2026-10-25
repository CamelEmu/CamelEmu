import 'package:flutter/material.dart';

import '../i18n/translations.g.dart';

/// A top-level destination of the app shell, shared by [AppNavBar] and [AppSideBar] so both
/// navigation styles list the same tabs in the same order.
class AppNavDestination {
  const AppNavDestination({
    required this.icon,
    required this.label,
    required this.outlinedIcon,
    required this.filledIcon,
  });

  final IconData icon;
  final String label;
  final IconData outlinedIcon;
  final IconData filledIcon;

  static List<AppNavDestination> of(BuildContext context) {
    final t = context.t;
    return [
      AppNavDestination(
        icon: Icons.videogame_asset,
        outlinedIcon: Icons.videogame_asset_outlined,
        filledIcon: Icons.videogame_asset,
        label: t.home.games,
      ),
      AppNavDestination(
        icon: Icons.more_horiz,
        outlinedIcon: Icons.settings_outlined,
        filledIcon: Icons.settings,
        label: t.home.options,
      ),
    ];
  }
}
