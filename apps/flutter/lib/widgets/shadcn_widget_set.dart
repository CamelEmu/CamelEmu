import 'package:flutter/material.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shadcn;

import 'app_nav_destination.dart';
import 'app_widgets.dart';

/// The [AppWidgetSet] drawn with shadcn_flutter.
class ShadcnWidgetSet extends AppWidgetSet {
  const ShadcnWidgetSet();

  @override
  Widget? textButton({
    required VoidCallback? onPressed,
    required Widget child,
    Widget? icon,
  }) {
    return shadcn.GhostButton(
      onPressed: onPressed,
      leading: icon,
      child: child,
    );
  }

  @override
  Widget? filledButton({
    required VoidCallback? onPressed,
    required Widget child,
    Widget? icon,
    bool tonal = false,
  }) {
    if (tonal) {
      return shadcn.SecondaryButton(
        onPressed: onPressed,
        leading: icon,
        child: child,
      );
    }
    return shadcn.PrimaryButton(
      onPressed: onPressed,
      leading: icon,
      child: child,
    );
  }

  @override
  Widget? iconButton({
    required Widget icon,
    required VoidCallback? onPressed,
    String? tooltip,
    bool tonal = false,
  }) {
    final button = tonal
        ? shadcn.IconButton.secondary(icon: icon, onPressed: onPressed)
        : shadcn.IconButton.ghost(icon: icon, onPressed: onPressed);
    if (tooltip == null) return button;
    return Tooltip(message: tooltip, child: button);
  }

  @override
  Widget? filterChip({
    required Widget label,
    required bool selected,
    required VoidCallback? onPressed,
    Widget? avatar,
  }) {
    return shadcn.Chip(
      leading: avatar,
      style: selected
          ? shadcn.ButtonVariance.primary
          : shadcn.ButtonVariance.secondary,
      onPressed: onPressed,
      child: label,
    );
  }

  @override
  Widget? listTile({
    Widget? leading,
    required Widget title,
    Widget? subtitle,
    Widget? trailing,
    VoidCallback? onTap,
    VoidCallback? onLongPress,
    bool enabled = true,
  }) {
    final content = shadcn.Basic(
      leading: leading,
      title: title,
      subtitle: subtitle,
      trailing: trailing,
    );
    if (onTap == null && onLongPress == null) {
      return Opacity(
        opacity: enabled ? 1 : 0.5,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: content,
        ),
      );
    }
    return shadcn.GhostButton(
      onPressed: enabled ? onTap : null,
      onLongPressUp: enabled ? onLongPress : null,
      alignment: Alignment.centerLeft,
      child: content,
    );
  }

  @override
  Widget? toggleSwitch({
    required bool value,
    required ValueChanged<bool> onChanged,
  }) {
    return shadcn.Switch(value: value, onChanged: onChanged);
  }

  @override
  Widget? select<T>({
    required List<String> labels,
    required List<T> values,
    required T value,
    required ValueChanged<T> onChanged,
  }) {
    if (!values.contains(value)) return null;
    return _ShadcnSelect<T>(
      labels: labels,
      values: values,
      value: value,
      onChanged: onChanged,
    );
  }

  @override
  Widget? slider({
    required double value,
    required double min,
    required double max,
    required ValueChanged<double> onChanged,
    int? divisions,
    ValueChanged<double>? onChangeEnd,
  }) {
    return shadcn.Slider(
      value: shadcn.SliderValue.single(value),
      min: min,
      max: max,
      divisions: divisions,
      onChanged: (changed) => onChanged(changed.value),
      onChangeEnd: onChangeEnd == null
          ? null
          : (ended) => onChangeEnd(ended.value),
    );
  }

  @override
  Widget? navigationBar({
    required List<AppNavDestination> destinations,
    required int selectedIndex,
    required ValueChanged<int> onSelected,
  }) {
    return Builder(
      builder: (context) {
        final theme = shadcn.Theme.of(context);
        final gap = theme.density.baseGap * theme.scaling;
        final contentPadding = theme.density.baseContentPadding * theme.scaling;
        return shadcn.NavigationBar(
          labelType: shadcn.NavigationLabelType.none,
          backgroundColor: theme.colorScheme.background,
          padding: EdgeInsets.fromLTRB(
            contentPadding,
            gap,
            contentPadding,
            gap + MediaQuery.paddingOf(context).bottom,
          ),
          expanded: true,
          children: [
            for (final (index, destination) in destinations.indexed)
              Expanded(
                child: shadcn.NavigationItem(
                  selected: index == selectedIndex,
                  onChanged: (_) => onSelected(index),
                  style: const shadcn.ButtonStyle.ghost(
                    density: shadcn.ButtonDensity.icon,
                  ),
                  selectedStyle: const shadcn.ButtonStyle.ghost(
                    density: shadcn.ButtonDensity.icon,
                  ),
                  child: Icon(
                    index == selectedIndex
                        ? destination.filledIcon
                        : destination.outlinedIcon,
                  ),
                ),
              ),
          ],
        );
      },
    );
  }

  @override
  Widget? card({required List<Widget> children}) {
    return shadcn.Card(
      padding: EdgeInsets.zero,
      child: Material(
        type: MaterialType.transparency,
        child: Column(
          children: [
            for (final (index, child) in children.indexed) ...[
              if (index > 0) const shadcn.Divider(),
              child,
            ],
          ],
        ),
      ),
    );
  }
}

/// A dropdown that shows its choices in a modal bottom sheet. The sheet is a route of the root
/// navigator, unlike the overlay of [shadcn.Select], so the back key closes it before it leaves
/// the page.
class _ShadcnSelect<T> extends StatelessWidget {
  const _ShadcnSelect({
    required this.labels,
    required this.values,
    required this.value,
    required this.onChanged,
  });

  final List<String> labels;
  final List<T> values;
  final T value;
  final ValueChanged<T> onChanged;

  Future<void> _open(BuildContext context) async {
    final picked = await showModalBottomSheet<T>(
      context: context,
      useRootNavigator: true,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (sheetContext) => SafeArea(
        top: false,
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var i = 0; i < values.length; i++)
                shadcn.GhostButton(
                  alignment: Alignment.centerLeft,
                  onPressed: () => Navigator.of(sheetContext).pop(values[i]),
                  trailing: values[i] == value
                      ? const Icon(Icons.check, size: 18)
                      : null,
                  child: Text(labels[i]),
                ),
            ],
          ),
        ),
      ),
    );
    if (picked != null && picked != value) onChanged(picked);
  }

  @override
  Widget build(BuildContext context) {
    return shadcn.OutlineButton(
      onPressed: () => _open(context),
      trailing: const Icon(Icons.unfold_more, size: 18),
      child: Text(labels[values.indexOf(value)]),
    );
  }
}
