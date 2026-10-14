import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../data/options/abstract_base_option.dart';
import '../../../../data/options/option_values_revision_provider.dart';
import '../../../../i18n/translations.g.dart';
import '../../../settings/widgets/toggle_settings_item.dart';
import 'option_commit.dart';

/// The tile of a [BoolOption]: a switch row. While [enabled] returns false the row is shown
/// faded and cannot be switched.
class BoolOptionTile extends ConsumerWidget {
  const BoolOptionTile({
    super.key,
    required this.option,
    this.enabled,
    required this.onAccessed,
    required this.onLongPress,
  });

  final BoolOption option;
  final bool Function(WidgetRef ref)? enabled;
  final VoidCallback onAccessed;
  final VoidCallback onLongPress;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    ref.watch(optionValuesRevisionProvider);
    final t = context.t;
    final isEnabled = enabled?.call(ref) ?? true;
    return ExcludeFocus(
      excluding: !isEnabled,
      child: IgnorePointer(
        ignoring: !isEnabled,
        child: Opacity(
          opacity: isEnabled ? 1 : 0.38,
          child: GestureDetector(
            behavior: HitTestBehavior.translucent,
            onLongPress: onLongPress,
            child: ToggleSettingsItem(
              icon: option.icon,
              title: option.title(t),
              subtitle: option.description?.call(t),
              value: option.value.read(ref),
              onChanged: (value) => ref.commitOptionValue(
                context,
                option.value,
                value,
                onAccessed,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
