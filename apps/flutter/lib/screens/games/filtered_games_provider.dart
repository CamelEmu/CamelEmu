import 'package:azahar_for_flutter/azahar_for_flutter.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/games/user_game_infos_provider.dart';
import '../../data/tags/tags_provider.dart';
import 'games_provider.dart';

/// The games matching the search text and every selected tag.
final filteredGamesProvider = Provider<List<Game>>((ref) {
  final query = ref.watch(gameQueryProvider).toLowerCase();
  final selectedTagIds = ref.watch(selectedTagIdsProvider).value ?? const {};
  final tags = ref.watch(tagsProvider).value ?? const [];
  var games = ref.watch(gamesProvider).value ?? const <Game>[];
  if (query.isNotEmpty) {
    final edited =
        ref.watch(userGameInfosProvider).value ?? const <String, String?>{};
    games = games.where((game) {
      final editedTitle = edited[game.path];
      final displayed = editedTitle != null && editedTitle.isNotEmpty
          ? editedTitle
          : game.title;
      return displayed.toLowerCase().contains(query) ||
          game.title.toLowerCase().contains(query);
    }).toList();
  }
  for (final tag in tags) {
    if (selectedTagIds.contains(tag.id)) games = tag.filter(games);
  }
  return games;
});
