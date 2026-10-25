import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../data/options/option_categories_provider.dart';
import '../../../data/options/pinned_options_provider.dart';
import '../../../i18n/translations.g.dart';
import 'clear_section_button.dart';
import 'emulator_load_section.dart';
import 'option_category_cards.dart';
import 'option_group_card.dart';

/// What the Options page shows when it is not searching: the pinned items, and then every
/// category. The pinned items are always shown, with a message while empty and a button at the
/// right of their heading that deletes everything in them after confirmation.
class OptionsHomeContent extends ConsumerWidget {
  const OptionsHomeContent({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final t = context.t;
    final index = ref.watch(optionEntryByIdProvider);
    final pinnedEntries = [
      for (final id in ref.watch(pinnedOptionsProvider).value ?? const [])
        ?index[id],
    ];
    final bottomInset = MediaQuery.paddingOf(context).bottom;
    return ListView(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomInset),
      children: [
        const EmulatorLoadSection(),
        OptionGroupCard(
          title: t.options.pinned,
          entries: pinnedEntries,
          emptyMessage: t.options.pinnedEmpty,
          trailing: ClearSectionButton(
            enabled: pinnedEntries.isNotEmpty,
            confirmTitle: t.options.clearPinnedTitle,
            confirmMessage: t.options.clearPinnedMessage,
            onConfirmed: () => ref.read(pinnedOptionsProvider.notifier).clear(),
          ),
        ),
        for (final category in ref.watch(optionCategoriesProvider))
          OptionCategoryCards(
            category: category,
            fallbackToCategoryTitle: true,
          ),
      ],
    );
  }
}
