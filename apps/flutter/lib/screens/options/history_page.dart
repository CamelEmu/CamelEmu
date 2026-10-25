import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/options/option_categories_provider.dart';
import '../../data/options/option_history_provider.dart';
import '../../i18n/translations.g.dart';
import '../../widgets/gamepad/gamepad_focus_region.dart';
import 'widgets/clear_section_button.dart';
import 'widgets/option_group_card.dart';

/// The recently used Options items, opened from the Options tab. The page title is the heading of
/// the section, and the button that deletes the whole history sits in the app bar.
class HistoryPage extends ConsumerWidget {
  const HistoryPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final t = context.t;
    final index = ref.watch(optionEntryByIdProvider);
    final entries = [
      for (final id in ref.watch(optionHistoryProvider).value ?? const [])
        ?index[id],
    ];
    final bottomInset = MediaQuery.paddingOf(context).bottom;
    return Scaffold(
      appBar: AppBar(
        title: Text(t.options.history),
        actions: [
          ClearSectionButton(
            iconOnly: true,
            enabled: entries.isNotEmpty,
            confirmTitle: t.options.clearHistoryTitle,
            confirmMessage: t.options.clearHistoryMessage,
            onConfirmed: () => ref.read(optionHistoryProvider.notifier).clear(),
          ),
          const SizedBox(width: 8),
        ],
      ),
      body: GamepadFocusRegion(
        child: ListView(
          padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomInset),
          children: [
            OptionGroupCard(
              entries: entries,
              emptyMessage: t.options.historyEmpty,
              inHistory: true,
            ),
          ],
        ),
      ),
    );
  }
}
