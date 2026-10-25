import 'package:flutter/material.dart';

import 'app_neumorphic.dart';

/// A flat panel: a [Material] filled with [fillColor] and outlined by [borderColor], or an
/// [AppNeumorphic] panel under the Legacy theme.
class AppPanel extends StatelessWidget {
  const AppPanel({
    super.key,
    required this.borderRadius,
    required this.fillColor,
    required this.borderColor,
    required this.child,
  });

  final BorderRadius borderRadius;
  final Color fillColor;
  final Color borderColor;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (AppNeumorphic.of(context) != null) {
      return AppNeumorphic(
        borderRadius: borderRadius,
        color: fillColor,
        borderColor: borderColor,
        child: child,
      );
    }

    return Material(
      color: fillColor,
      shape: RoundedRectangleBorder(
        borderRadius: borderRadius,
        side: BorderSide(color: borderColor),
      ),
      clipBehavior: Clip.antiAlias,
      child: child,
    );
  }
}
