import 'package:flutter/material.dart';

import '../../../widgets/app_neumorphic.dart';
import '../../games/widgets/game_icon.dart';

class EmulationGameIcon extends StatelessWidget {
  const EmulationGameIcon({
    super.key,
    required this.iconPath,
    required this.size,
    required this.radius,
  });

  final String? iconPath;
  final double size;
  final double radius;

  @override
  Widget build(BuildContext context) {
    final borderRadius = BorderRadius.circular(radius);
    final icon = SizedBox(
      width: size,
      height: size,
      child: GameIcon(iconPath: iconPath),
    );
    if (AppNeumorphic.of(context) != null) {
      return AppNeumorphic(borderRadius: borderRadius, child: icon);
    }
    return ClipRRect(borderRadius: borderRadius, child: icon);
  }
}
