import 'package:azahar_for_flutter/azahar_for_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../data/tags/tags_provider.dart';
import '../../../widgets/app_widgets.dart';
import 'tag_color_dot.dart';

/// Lets the user attach any number of tags to [game].
class GameTagSelector extends ConsumerWidget {
  const GameTagSelector({super.key, required this.game});

  final Game game;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tags = ref.watch(tagsProvider).value ?? const [];
    return Align(
      alignment: Alignment.centerLeft,
      child: Wrap(
        spacing: 8,
        runSpacing: 4,
        children: [
          for (final tag in tags)
            AppFilterChip(
              avatar: TagColorDot(color: tag.color),
              label: Text(tag.name(context)),
              selected: tag.assignedPaths.contains(game.path),
              onPressed: () =>
                  ref.read(tagsProvider.notifier).toggle(game.path, tag.id),
            ),
        ],
      ),
    );
  }
}
