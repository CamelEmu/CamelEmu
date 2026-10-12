import 'package:flutter/material.dart';

import '../../../data/settings/country.dart';
import '../../../i18n/translations.g.dart';

class ArticCountryMismatchDialog {
  const ArticCountryMismatchDialog._();

  static const _regionLabels = [
    'JPN',
    'USA',
    'EUR',
    'AUS',
    'CHN',
    'KOR',
    'TWN',
  ];

  static Future<bool> show(
    BuildContext context, {
    required int region,
    required Country? current,
    required Country replacement,
  }) async {
    final t = context.t;
    final override = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (context) => PopScope(
        canPop: false,
        child: AlertDialog(
          title: Text(t.emulation.articCountryMismatchTitle),
          content: Text(
            t.emulation.articCountryMismatchMessage(
              region: region < _regionLabels.length
                  ? _regionLabels[region]
                  : region.toString(),
              current: current?.label(t) ?? '-',
              replacement: replacement.label(t),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(t.emulation.articCountryKeep),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(t.emulation.articCountryOverride),
            ),
          ],
        ),
      ),
    );
    return override ?? false;
  }
}
