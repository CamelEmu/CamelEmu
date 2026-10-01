import 'package:azahar_for_flutter/azahar_for_flutter.dart';
import 'package:flutter/material.dart';

import '../../../widgets/long_press_menu_sheet.dart';
import 'about_game_content.dart';

/// Mirrors the Compose client's `AboutGameBottomSheet`: shown on a long press of a game card on
/// mobile platforms.
class AboutGameBottomSheet extends StatelessWidget {
  const AboutGameBottomSheet({
    super.key,
    required this.game,
    required this.onPlay,
    required this.onUninstalled,
  });

  final Game game;
  final VoidCallback onPlay;
  final VoidCallback onUninstalled;

  static Future<void> show(
    BuildContext context, {
    required Game game,
    required VoidCallback onPlay,
    required VoidCallback onUninstalled,
  }) {
    return LongPressMenuSheet.show(
      context,
      builder: (_) => AboutGameBottomSheet(
        game: game,
        onPlay: onPlay,
        onUninstalled: onUninstalled,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return LongPressMenuSheet(
      child: AboutGameContent(
        game: game,
        onPlay: onPlay,
        onUninstalled: onUninstalled,
      ),
    );
  }
}
