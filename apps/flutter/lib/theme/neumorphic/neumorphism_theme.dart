import 'package:flutter/material.dart';
import 'package:flutter_neumorphic/flutter_neumorphic.dart'
    show
        Neumorphic,
        NeumorphicBoxShape,
        NeumorphicColors,
        NeumorphicShape,
        NeumorphicStyle,
        NeumorphicTheme,
        NeumorphicThemeData;

import '../azahar/azahar_theme.dart';
import '../extensions/app_navigation_bar_theme.dart';
import '../extensions/app_search_bar_theme.dart';
import '../extensions/app_toggle_switch_theme.dart';
import '../extensions/background_blob_theme.dart';
import '../extensions/emulation_focus_frame_theme.dart';
import '../extensions/game_card_theme.dart';
import '../extensions/gamepad_notification_bar_theme.dart';
import '../extensions/glass_surface_theme.dart';
import '../extensions/neumorphic_surface_theme.dart';
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
    final shadowColor = brightness == Brightness.dark
        ? Colors.black
        : const Color(0xFFA3B1C6);
    return ThemeData(
      useMaterial3: true,
      colorScheme: colorScheme,
      scaffoldBackgroundColor: base,
      cardTheme: CardThemeData(
        color: base,
        surfaceTintColor: Colors.transparent,
        shadowColor: shadowColor,
        elevation: 4,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
      ),
      appBarTheme: AppBarTheme(
        backgroundColor: base,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: base,
        surfaceTintColor: Colors.transparent,
        shadowColor: shadowColor,
        elevation: 8,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(24)),
      ),
      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: base,
        surfaceTintColor: Colors.transparent,
        shadowColor: shadowColor,
        elevation: 8,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.vertical(top: Radius.circular(28)),
        ),
      ),
      popupMenuTheme: PopupMenuThemeData(
        color: base,
        surfaceTintColor: Colors.transparent,
        shadowColor: shadowColor,
        elevation: 6,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: _buttonStyle(
          colorScheme,
          fill: colorScheme.primary,
          foreground: colorScheme.onPrimary,
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: _buttonStyle(
          colorScheme,
          fill: base,
          foreground: colorScheme.primary,
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: _buttonStyle(
          colorScheme,
          fill: base,
          foreground: colorScheme.primary,
        ),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: _buttonStyle(
          colorScheme,
          fill: base,
          foreground: colorScheme.primary,
        ),
      ),
      iconButtonTheme: IconButtonThemeData(
        style: _buttonStyle(
          colorScheme,
          fill: base,
          foreground: colorScheme.onSurfaceVariant,
          radius: BorderRadius.circular(999),
          plainInBackButton: true,
        ),
      ),
      floatingActionButtonTheme: FloatingActionButtonThemeData(
        backgroundColor: base,
        foregroundColor: colorScheme.primary,
        shape: const CircleBorder(),
        elevation: 6,
        highlightElevation: 2,
      ),
      chipTheme: ChipThemeData(
        backgroundColor: base,
        selectedColor: base,
        surfaceTintColor: Colors.transparent,
        shadowColor: shadowColor,
        elevation: 3,
        pressElevation: 1,
        side: BorderSide.none,
      ),
      extensions: _extensions(colorScheme),
    );
  }

  static ButtonStyle _buttonStyle(
    ColorScheme colorScheme, {
    required Color fill,
    required Color foreground,
    BorderRadius? radius,
    bool plainInBackButton = false,
  }) {
    final shape = radius ?? BorderRadius.circular(16);
    return ButtonStyle(
      elevation: const WidgetStatePropertyAll(0),
      backgroundColor: const WidgetStatePropertyAll(Colors.transparent),
      surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
      shadowColor: const WidgetStatePropertyAll(Colors.transparent),
      side: const WidgetStatePropertyAll(BorderSide.none),
      overlayColor: WidgetStatePropertyAll(foreground.withValues(alpha: 0.08)),
      shape: WidgetStatePropertyAll(RoundedRectangleBorder(borderRadius: shape)),
      foregroundColor: WidgetStateProperty.resolveWith(
        (states) => states.contains(WidgetState.disabled)
            ? colorScheme.onSurface.withValues(alpha: 0.38)
            : foreground,
      ),
      backgroundBuilder: (context, states, child) {
        if (plainInBackButton &&
            context.findAncestorWidgetOfExactType<BackButton>() != null) {
          return child!;
        }
        final disabled = states.contains(WidgetState.disabled);
        final pressed = states.contains(WidgetState.pressed);
        return Neumorphic(
          drawSurfaceAboveChild: false,
          style: NeumorphicStyle(
            shape: NeumorphicShape.flat,
            depth: disabled ? 0 : (pressed ? -3 : 4),
            color: disabled ? colorScheme.surface : fill,
            boxShape: NeumorphicBoxShape.roundRect(shape),
          ),
          child: child,
        );
      },
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
      const NeumorphicSurfaceTheme(depth: 4, intensity: 0.7),
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
