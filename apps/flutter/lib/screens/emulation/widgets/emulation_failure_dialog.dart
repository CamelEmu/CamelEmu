import 'package:flutter/material.dart';

import '../../../i18n/translations.g.dart';

class EmulationFailureDialog {
  const EmulationFailureDialog._();

  static Future<void> show(BuildContext context, String message) {
    final t = context.t;
    return showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => PopScope(
        canPop: false,
        child: AlertDialog(
          title: Text(t.emulation.failureTitle),
          content: Text(message),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: Text(MaterialLocalizations.of(context).okButtonLabel),
            ),
          ],
        ),
      ),
    );
  }
}
