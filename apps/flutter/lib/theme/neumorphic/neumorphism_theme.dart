import 'package:flutter/material.dart';
import 'package:flutter_neumorphic/flutter_neumorphic.dart'
    show NeumorphicColors, NeumorphicTheme, NeumorphicThemeData;

import '../azahar/azahar_theme.dart';
import '../extensions/app_navigation_bar_theme.dart';
import '../extensions/app_search_bar_theme.dart';
import '../extensions/app_toggle_switch_theme.dart';
import '../extensions/background_blob_theme.dart';
import '../extensions/emulation_focus_frame_theme.dart';
import '../extensions/game_card_theme.dart';
import '../extensions/gamepad_notification_bar_theme.dart';
import '../extensions/glass_surface_theme.dart';
import '../extensions/settings_list_item_theme.dart';

abstract final class NeumorphismTheme {
  static const Color _lightBase = Color(0xFFE0E5EC);
  static const Color _darkBase = Color(0xFF2B2E33);
  static const Color _defaultSeed = Colors.blueGrey;

  static ThemeData light({
    int? staticThemeColor,
    bool materialYou = false,
    ColorScheme? dynamicScheme,
  }) {
    return _themeData(
      brightness: Brightness.light,
      staticThemeColor: staticThemeColor,
      materialYou: materialYou,
      dynamicScheme: dynamicScheme,
    );
  }

  static ThemeData dark({
    int? staticThemeColor,
    bool blackBackgrounds = false,
    bool materialYou = false,
    ColorScheme? dynamicScheme,
  }) {
    return _themeData(
      brightness: Brightness.dark,
      staticThemeColor: staticThemeColor,
      blackBackgrounds: blackBackgrounds,
      materialYou: materialYou,
      dynamicScheme: dynamicScheme,
    );
  }

  static NeumorphicThemeData neumorphicData(ThemeData theme) {
    final scheme = theme.colorScheme;
    final dark = scheme.brightness == Brightness.dark;
    return NeumorphicThemeData(
      baseColor: scheme.surface,
      accentColor: scheme.primary,
      variantColor: scheme.secondary,
      disabledColor: scheme.onSurface.withValues(alpha: 0.38),
      defaultTextColor: scheme.onSurface,
      shadowLightColor: dark
          ? NeumorphicColors.decorationMaxWhiteColor.withValues(alpha: 0.12)
          : NeumorphicColors.decorationMaxWhiteColor,
      shadowDarkColor: dark
          ? Colors.black
          : NeumorphicColors.decorationMaxDarkColor,
      iconTheme: IconThemeData(color: scheme.onSurfaceVariant),
    );
  }

  static ThemeData _themeData({
    required Brightness brightness,
    int? staticThemeColor,
    bool blackBackgrounds = false,
    bool materialYou = false,
    ColorScheme? dynamicScheme,
  }) {
    final base = switch (brightness) {
      Brightness.light => _lightBase,
      Brightness.dark => blackBackgrounds ? Colors.black : _darkBase,
    };
    final source = materialYou && dynamicScheme != null
        ? dynamicScheme
        : ColorScheme.fromSeed(
            seedColor: staticThemeColor == null
                ? _defaultSeed
                : AzaharTheme.staticThemeColors[staticThemeColor],
            brightness: brightness,
          );
    final colorScheme = source.copyWith(
      surface: base,
      surfaceContainerLowest: base,
      surfaceContainerLow: base,
      surfaceContainer: base,
      surfaceContainerHigh: base,
      surfaceContainerHighest: base,
    );
    return ThemeData(
      useMaterial3: true,
      colorScheme: colorScheme,
      scaffoldBackgroundColor: base,
      extensions: _extensions(colorScheme),
    );
  }

  static List<BoxShadow> _raised(ColorScheme scheme, {double distance = 5}) {
    final dark = scheme.brightness == Brightness.dark;
    return [
      BoxShadow(
        color: dark
            ? Colors.white.withValues(alpha: 0.05)
            : Colors.white.withValues(alpha: 0.8),
        offset: Offset(-distance, -distance),
        blurRadius: distance * 2,
      ),
      BoxShadow(
        color: dark
            ? Colors.black.withValues(alpha: 0.6)
            : const Color(0xFFA3B1C6).withValues(alpha: 0.6),
        offset: Offset(distance, distance),
        blurRadius: distance * 2,
      ),
    ];
  }

  static List<ThemeExtension<dynamic>> _extensions(ColorScheme colorScheme) {
    return [
      GlassSurfaceTheme.flat(colorScheme).copyWith(
        fillColor: colorScheme.surface,
        borderRadius: BorderRadius.circular(20),
        shadow: _raised(colorScheme),
      ),
      AppNavigationBarTheme.flat(colorScheme),
      AppSearchBarTheme.flat(colorScheme),
      SettingsListItemTheme.flat(colorScheme),
      AppToggleSwitchTheme.flat(colorScheme),
      GameCardTheme.flat(colorScheme),
      BackgroundBlobTheme.flat(colorScheme),
      GamepadNotificationBarTheme.flat(colorScheme),
      EmulationFocusFrameTheme.flat(colorScheme),
    ];
  }
}

class NeumorphismScope extends StatelessWidget {
  const NeumorphismScope({
    super.key,
    required this.themeMode,
    required this.lightTheme,
    required this.darkTheme,
    required this.child,
  });

  final ThemeMode themeMode;
  final ThemeData lightTheme;
  final ThemeData darkTheme;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return NeumorphicTheme(
      themeMode: themeMode,
      theme: NeumorphismTheme.neumorphicData(lightTheme),
      darkTheme: NeumorphismTheme.neumorphicData(darkTheme),
      child: child,
    );
  }
}
