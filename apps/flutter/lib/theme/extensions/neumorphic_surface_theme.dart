import 'package:flutter/material.dart';

class NeumorphicSurfaceTheme extends ThemeExtension<NeumorphicSurfaceTheme> {
  const NeumorphicSurfaceTheme({required this.depth, required this.intensity});

  final double depth;
  final double intensity;

  @override
  NeumorphicSurfaceTheme copyWith({double? depth, double? intensity}) {
    return NeumorphicSurfaceTheme(
      depth: depth ?? this.depth,
      intensity: intensity ?? this.intensity,
    );
  }

  @override
  NeumorphicSurfaceTheme lerp(
    ThemeExtension<NeumorphicSurfaceTheme>? other,
    double t,
  ) {
    if (other is! NeumorphicSurfaceTheme) return this;
    return NeumorphicSurfaceTheme(
      depth: depth + (other.depth - depth) * t,
      intensity: intensity + (other.intensity - intensity) * t,
    );
  }
}
