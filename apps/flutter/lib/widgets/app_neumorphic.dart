import 'package:flutter/material.dart';
import 'package:flutter_neumorphic/flutter_neumorphic.dart'
    show
        Neumorphic,
        NeumorphicBorder,
        NeumorphicBoxShape,
        NeumorphicShape,
        NeumorphicStyle;

import '../theme/extensions/neumorphic_surface_theme.dart';

class AppNeumorphic extends StatelessWidget {
  const AppNeumorphic({
    super.key,
    required this.borderRadius,
    required this.child,
    this.color,
    this.borderColor,
    this.inset = false,
  });

  final BorderRadius borderRadius;
  final Widget child;
  final Color? color;
  final Color? borderColor;
  final bool inset;

  static NeumorphicSurfaceTheme? of(BuildContext context) {
    return Theme.of(context).extension<NeumorphicSurfaceTheme>();
  }

  @override
  Widget build(BuildContext context) {
    final theme = of(context)!;
    final border = borderColor;
    final fill = color;
    final surface = Theme.of(context).colorScheme.surface;
    return Neumorphic(
      drawSurfaceAboveChild: false,
      style: NeumorphicStyle(
        shape: NeumorphicShape.flat,
        depth: inset ? -theme.depth : theme.depth,
        intensity: theme.intensity,
        color: fill == null ? null : Color.alphaBlend(fill, surface),
        border: border == null || border.a == 0
            ? const NeumorphicBorder.none()
            : NeumorphicBorder(color: border, width: 1),
        boxShape: NeumorphicBoxShape.roundRect(borderRadius),
      ),
      child: Material(
        type: MaterialType.transparency,
        borderRadius: borderRadius,
        clipBehavior: Clip.antiAlias,
        child: child,
      ),
    );
  }
}
