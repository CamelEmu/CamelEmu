import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../theme/extensions/glass_surface_theme.dart';
import '../../../widgets/app_panel.dart';
import '../../../widgets/app_widgets.dart';
import 'tag_color_dot.dart';

/// A single glass chip showing the tag [color] as a dot. When [selected] it is highlighted with the
/// theme's primary color, the same for every tag. It cannot be tapped while [onTap] is null.
class TagChip extends ConsumerWidget {
  const TagChip({
    super.key,
    required this.label,
    required this.color,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final Color color;
  final bool selected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final custom = AppWidgets.of(context).filterChip(
      label: Text(label),
      avatar: TagColorDot(color: color),
      selected: selected,
      onPressed: onTap,
    );
    if (custom != null) {
      return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
        child: custom,
      );
    }
    final surfaceTheme = Theme.of(context).extension<GlassSurfaceTheme>()!;
    final highlight = Theme.of(context).colorScheme.primary;
    final radius = BorderRadius.circular(20);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
      child: AppPanel(
        borderRadius: radius,
        fillColor: selected
            ? highlight.withValues(alpha: 0.35)
            : surfaceTheme.fillColor,
        borderColor: selected ? highlight : surfaceTheme.borderColor,
        child: InkWell(
          onTap: onTap,
          borderRadius: radius,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 14),
            child: Center(
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  TagColorDot(color: color),
                  const SizedBox(width: 8),
                  Text(label),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
