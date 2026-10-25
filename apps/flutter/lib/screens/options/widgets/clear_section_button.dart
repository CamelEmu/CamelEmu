import 'package:flutter/material.dart';

import '../../../i18n/translations.g.dart';
import '../../../widgets/app_widgets.dart';
import '../../../widgets/confirmation_dialog.dart';

/// The delete button at the right end of the History and Pinned headings: a trash icon beside a
/// label. Pressing it asks for confirmation and then calls [onConfirmed]; it is disabled while
/// there is nothing to delete. With [iconOnly] the label is left out and only the icon is shown.
class ClearSectionButton extends StatelessWidget {
  const ClearSectionButton({
    super.key,
    required this.confirmTitle,
    required this.confirmMessage,
    required this.onConfirmed,
    this.enabled = true,
    this.iconOnly = false,
  });

  final String confirmTitle;
  final String confirmMessage;
  final VoidCallback onConfirmed;
  final bool enabled;
  final bool iconOnly;

  @override
  Widget build(BuildContext context) {
    final label = context.t.options.clear;
    final onPressed = enabled
        ? () async {
            final confirmed = await ConfirmationDialog.show(
              context,
              title: confirmTitle,
              message: confirmMessage,
              confirmLabel: label,
            );
            if (confirmed) onConfirmed();
          }
        : null;
    if (iconOnly) {
      return AppIconButton(
        onPressed: onPressed,
        icon: const Icon(Icons.delete_outline),
        tooltip: label,
      );
    }
    return AppTextButton(
      onPressed: onPressed,
      icon: const Icon(Icons.delete_outline),
      child: Text(label),
    );
  }
}
