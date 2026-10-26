import 'package:flutter/material.dart';

import '../../../widgets/app_widgets.dart';

/// One action of the in-game menu: a [ListTile] normally, or just a tooltipped icon button when
/// [iconsOnly] is set.
class EmulationMenuItem extends StatelessWidget {
  const EmulationMenuItem({
    super.key,
    required this.icon,
    required this.title,
    required this.onTap,
    this.onLongPress,
    this.iconsOnly = false,
    this.enabled = true,
  });

  final IconData icon;
  final String title;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final bool iconsOnly;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    if (iconsOnly) {
      return Align(
        child: GestureDetector(
          onLongPress: enabled ? onLongPress : null,
          child: AppIconButton(
            icon: Icon(icon),
            tooltip: title,
            onPressed: enabled ? onTap : null,
          ),
        ),
      );
    }
    return AppListTile(
      enabled: enabled,
      leading: Icon(icon),
      title: Text(title),
      onTap: onTap,
      onLongPress: onLongPress,
    );
  }
}
