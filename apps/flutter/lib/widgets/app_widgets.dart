import 'package:flutter/material.dart';

import 'app_nav_destination.dart';

/// The widgets that can be swapped for another UI library without their callers knowing. Each
/// method returns null to keep the Material widget, which [AppTextButton] and the other widgets
/// in this file then build themselves. The app root picks the set once, so no widget has to look
/// at the theme style.
class AppWidgetSet {
  const AppWidgetSet();

  Widget? textButton({
    required VoidCallback? onPressed,
    required Widget child,
    Widget? icon,
  }) => null;

  Widget? filledButton({
    required VoidCallback? onPressed,
    required Widget child,
    Widget? icon,
    bool tonal = false,
  }) => null;

  Widget? iconButton({
    required Widget icon,
    required VoidCallback? onPressed,
    String? tooltip,
    bool tonal = false,
  }) => null;

  Widget? filterChip({
    required Widget label,
    required bool selected,
    required VoidCallback? onPressed,
    Widget? avatar,
  }) => null;

  Widget? listTile({
    Widget? leading,
    required Widget title,
    Widget? subtitle,
    Widget? trailing,
    VoidCallback? onTap,
    VoidCallback? onLongPress,
    bool enabled = true,
  }) => null;

  Widget? toggleSwitch({
    required bool value,
    required ValueChanged<bool> onChanged,
  }) => null;

  Widget? select<T>({
    required List<String> labels,
    required List<T> values,
    required T value,
    required ValueChanged<T> onChanged,
  }) => null;

  Widget? slider({
    required double value,
    required double min,
    required double max,
    required ValueChanged<double> onChanged,
    int? divisions,
    ValueChanged<double>? onChangeEnd,
  }) => null;

  Widget? circularProgress({double? size, double? strokeWidth}) => null;

  Widget? linearProgress({
    double? value,
    double? minHeight,
    Color? color,
    Color? backgroundColor,
    BorderRadius? borderRadius,
  }) => null;

  Widget? navigationBar({
    required List<AppNavDestination> destinations,
    required int selectedIndex,
    required ValueChanged<int> onSelected,
  }) => null;

  Widget? card({required List<Widget> children}) => null;
}

/// Provides the [AppWidgetSet] the widgets below the app root are built with. Without it the
/// Material widgets are used.
class AppWidgets extends InheritedWidget {
  const AppWidgets({super.key, required this.set, required super.child});

  final AppWidgetSet set;

  static AppWidgetSet of(BuildContext context) {
    return context.dependOnInheritedWidgetOfExactType<AppWidgets>()?.set ??
        const AppWidgetSet();
  }

  @override
  bool updateShouldNotify(AppWidgets oldWidget) => oldWidget.set != set;
}

class AppTextButton extends StatelessWidget {
  const AppTextButton({
    super.key,
    required this.onPressed,
    required this.child,
    this.icon,
  });

  final VoidCallback? onPressed;
  final Widget child;
  final Widget? icon;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(
      context,
    ).textButton(onPressed: onPressed, child: child, icon: icon);
    if (custom != null) return custom;
    final leading = icon;
    if (leading == null) return TextButton(onPressed: onPressed, child: child);
    return TextButton.icon(onPressed: onPressed, icon: leading, label: child);
  }
}

class AppFilledButton extends StatelessWidget {
  const AppFilledButton({
    super.key,
    required this.onPressed,
    required this.child,
    this.icon,
    this.tonal = false,
  });

  final VoidCallback? onPressed;
  final Widget child;
  final Widget? icon;
  final bool tonal;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(context).filledButton(
      onPressed: onPressed,
      child: child,
      icon: icon,
      tonal: tonal,
    );
    if (custom != null) return custom;
    final leading = icon;
    if (leading == null) {
      return tonal
          ? FilledButton.tonal(onPressed: onPressed, child: child)
          : FilledButton(onPressed: onPressed, child: child);
    }
    return tonal
        ? FilledButton.tonalIcon(
            onPressed: onPressed,
            icon: leading,
            label: child,
          )
        : FilledButton.icon(onPressed: onPressed, icon: leading, label: child);
  }
}

class AppIconButton extends StatelessWidget {
  const AppIconButton({
    super.key,
    required this.icon,
    required this.onPressed,
    this.tooltip,
    this.tonal = false,
  });

  final Widget icon;
  final VoidCallback? onPressed;
  final String? tooltip;
  final bool tonal;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(context).iconButton(
      icon: icon,
      onPressed: onPressed,
      tooltip: tooltip,
      tonal: tonal,
    );
    if (custom != null) return custom;
    return tonal
        ? IconButton.filledTonal(
            tooltip: tooltip,
            icon: icon,
            onPressed: onPressed,
          )
        : IconButton(tooltip: tooltip, icon: icon, onPressed: onPressed);
  }
}

/// A selectable chip, highlighted with the theme's primary color when [selected].
class AppFilterChip extends StatelessWidget {
  const AppFilterChip({
    super.key,
    required this.label,
    required this.selected,
    required this.onPressed,
    this.avatar,
  });

  final Widget label;
  final Widget? avatar;
  final bool selected;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(context).filterChip(
      label: label,
      selected: selected,
      onPressed: onPressed,
      avatar: avatar,
    );
    if (custom != null) return custom;
    final highlight = Theme.of(context).colorScheme.primary;
    return FilterChip(
      avatar: avatar,
      showCheckmark: false,
      label: label,
      selected: selected,
      selectedColor: highlight.withValues(alpha: 0.35),
      side: BorderSide(
        color: selected
            ? highlight
            : Theme.of(context).colorScheme.outlineVariant,
      ),
      onSelected: onPressed == null ? null : (_) => onPressed!(),
    );
  }
}

/// A slider over [min]..[max]. [label] is shown above the thumb while it is dragged, by the
/// widgets that support it.
class AppSlider extends StatelessWidget {
  const AppSlider({
    super.key,
    required this.value,
    required this.min,
    required this.max,
    required this.onChanged,
    this.divisions,
    this.label,
    this.onChangeEnd,
  });

  final double value;
  final double min;
  final double max;
  final int? divisions;
  final String? label;
  final ValueChanged<double> onChanged;
  final ValueChanged<double>? onChangeEnd;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(context).slider(
      value: value,
      min: min,
      max: max,
      divisions: divisions,
      onChanged: onChanged,
      onChangeEnd: onChangeEnd,
    );
    if (custom != null) return custom;
    return Slider(
      value: value,
      min: min,
      max: max,
      divisions: divisions,
      label: label,
      onChanged: onChanged,
      onChangeEnd: onChangeEnd,
    );
  }
}

class AppCircularProgress extends StatelessWidget {
  const AppCircularProgress({super.key, this.size, this.strokeWidth});

  final double? size;
  final double? strokeWidth;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(
      context,
    ).circularProgress(size: size, strokeWidth: strokeWidth);
    if (custom != null) return custom;
    final indicator = strokeWidth == null
        ? const CircularProgressIndicator()
        : CircularProgressIndicator(strokeWidth: strokeWidth!);
    if (size == null) return indicator;
    return SizedBox.square(dimension: size, child: indicator);
  }
}

class AppLinearProgress extends StatelessWidget {
  const AppLinearProgress({
    super.key,
    this.value,
    this.minHeight,
    this.color,
    this.backgroundColor,
    this.borderRadius,
  });

  final double? value;
  final double? minHeight;
  final Color? color;
  final Color? backgroundColor;
  final BorderRadius? borderRadius;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(context).linearProgress(
      value: value,
      minHeight: minHeight,
      color: color,
      backgroundColor: backgroundColor,
      borderRadius: borderRadius,
    );
    if (custom != null) return custom;
    return LinearProgressIndicator(
      value: value,
      minHeight: minHeight,
      color: color,
      backgroundColor: backgroundColor,
      borderRadius: borderRadius,
    );
  }
}

class AppListTile extends StatelessWidget {
  const AppListTile({
    super.key,
    this.leading,
    required this.title,
    this.subtitle,
    this.trailing,
    this.onTap,
    this.onLongPress,
    this.enabled = true,
  });

  final Widget? leading;
  final Widget title;
  final Widget? subtitle;
  final Widget? trailing;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final custom = AppWidgets.of(context).listTile(
      leading: leading,
      title: title,
      subtitle: subtitle,
      trailing: trailing,
      onTap: onTap,
      onLongPress: onLongPress,
      enabled: enabled,
    );
    if (custom != null) return custom;
    return ListTile(
      enabled: enabled,
      leading: leading,
      title: title,
      subtitle: subtitle,
      trailing: trailing,
      onTap: onTap,
      onLongPress: onLongPress,
    );
  }
}
