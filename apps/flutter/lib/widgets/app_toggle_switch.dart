import 'package:flutter/material.dart';
import 'package:flutter_neumorphic/flutter_neumorphic.dart'
    show NeumorphicShape, NeumorphicSwitch, NeumorphicSwitchStyle;

import '../theme/extensions/app_toggle_switch_theme.dart';
import 'app_neumorphic.dart';
import 'app_widgets.dart';

/// A pill-shaped toggle switch. Its colors and size come entirely from [AppToggleSwitchTheme],
/// so this single widget renders both the Azahar and Legacy looks.
class AppToggleSwitch extends StatelessWidget {
  const AppToggleSwitch({
    super.key,
    required this.value,
    required this.onChanged,
  });

  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(
      context,
    ).toggleSwitch(value: value, onChanged: onChanged);
    if (custom != null) return custom;
    final theme = Theme.of(context).extension<AppToggleSwitchTheme>()!;
    if (AppNeumorphic.of(context) != null) {
      final scheme = Theme.of(context).colorScheme;
      return NeumorphicSwitch(
        value: value,
        onChanged: onChanged,
        height: theme.trackSize.height,
        style: NeumorphicSwitchStyle(
          activeTrackColor: scheme.primary,
          inactiveTrackColor: scheme.surface,
          activeThumbColor: scheme.surface,
          inactiveThumbColor: scheme.surface,
          thumbShape: NeumorphicShape.flat,
        ),
      );
    }
    return GestureDetector(
      onTap: () => onChanged(!value),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        width: theme.trackSize.width,
        height: theme.trackSize.height,
        padding: const EdgeInsets.symmetric(horizontal: 2),
        decoration: BoxDecoration(
          color: value ? theme.trackColorOn : theme.trackColorOff,
          borderRadius: BorderRadius.circular(theme.trackSize.height / 2),
          border: value ? null : Border.all(color: theme.trackBorderColorOff),
        ),
        alignment: value ? Alignment.centerRight : Alignment.centerLeft,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          width: theme.thumbSize,
          height: theme.thumbSize,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: value ? theme.thumbColorOn : theme.thumbColorOff,
          ),
        ),
      ),
    );
  }
}
