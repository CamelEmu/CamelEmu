import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app_services.dart';
import '../emulation/emulation_page.dart';
import 'games_provider.dart';
import 'package:azahar_for_flutter/azahar_for_flutter.dart';

final gameProcessProvider =
    NotifierProvider.autoDispose<GameProcessNotifier, bool>(
      GameProcessNotifier.new,
    );

/// Tracks whether a game is currently running.
///
/// The game runs in this process: the emulation screen is pushed onto the navigator and the
/// session is released when the screen is left, so another game can be started afterwards.
///
/// The screen is pushed at once. What has to finish before the core starts is handed to the
/// screen, which waits for it while its loading overlay is shown.
class GameProcessNotifier extends Notifier<bool> {
  bool _disposed = false;

  @override
  bool build() {
    ref.onDispose(() => _disposed = true);
    return false;
  }

  /// Starts the game at [path], which is [game] when it is a listed game.
  Future<void> launch(
    BuildContext context, {
    required String path,
    Game? game,
  }) async {
    if (state) return;
    final navigator = Navigator.of(context, rootNavigator: true);
    final games = ref.read(gamesProvider.notifier);
    state = true;
    final scansHeld = games.holdScans();
    if (game != null) {
      unawaited(_markLastPlayed(game));
    }
    try {
      await navigator.push(
        MaterialPageRoute<void>(
          builder: (_) =>
              EmulationPage(gamePath: path, game: game, ready: scansHeld),
        ),
      );
    } finally {
      games.releaseScans();
      if (!_disposed) state = false;
    }
  }

  Future<void> _markLastPlayed(Game game) async {
    try {
      await AppServices.gameRepository.markLastPlayed(game.path);
    } catch (error) {
      debugPrint('Recording the last played time failed: $error');
    }
  }
}
