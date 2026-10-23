import 'dart:ui';

import 'package:flutter/material.dart';

import 'app_neumorphic.dart';

class AppLiquidGlass extends StatelessWidget {
  const AppLiquidGlass({
    super.key,
    required this.borderRadius,
    required this.blurSigma,
    required this.fillColor,
    required this.borderColor,
    required this.child,
    this.backdropBlur = true,
    this.shadow = const [],
  });

  final BorderRadius borderRadius;
  final double blurSigma;
  final Color fillColor;
  final Color borderColor;
  final bool backdropBlur;
  final List<BoxShadow> shadow;
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

    final shape = RoundedRectangleBorder(
      borderRadius: borderRadius,
      side: BorderSide(color: borderColor),
    );

    Widget surface = Material(
      color: fillColor,
      shape: shape,
      clipBehavior: Clip.antiAlias,
      child: child,
    );

    if (blurSigma > 0 && backdropBlur) {
      surface = ClipRRect(
        borderRadius: borderRadius,
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: blurSigma, sigmaY: blurSigma),
          child: surface,
        ),
      );
    }

    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: borderRadius,
        boxShadow: shadow,
      ),
      child: surface,
    );
  }
}
