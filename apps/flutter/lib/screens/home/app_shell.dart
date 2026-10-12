import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../data/gamepad/shell_navigation_provider.dart';
import '../../data/platform_provider.dart';
import '../../data/settings/accessibility_settings_provider.dart';
import '../../data/settings/advanced_settings_provider.dart';
import '../../data/settings/animation_speed.dart';
import '../../theme/extensions/app_navigation_bar_theme.dart';
import '../../theme/extensions/background_blob_theme.dart';
import '../../theme/theme_settings_provider.dart';
import '../../theme/theme_style.dart';
import '../../widgets/app_nav_bar.dart';
import '../../widgets/app_nav_destination.dart';
import '../../widgets/background_blobs.dart';
import '../../widgets/gamepad/gamepad_focus_region.dart';
import '../../widgets/gamepad_notification_bar.dart';
import 'desktop_app_shell.dart';

/// The bottom navigation shell across the Games/Options tabs, mirroring the original app's
/// `MainScreen` + `MainBottomNavigation`. Every other screen (emulation, settings, ...) lives
/// outside this shell. [AppNavBar]'s shape (floating pill vs. flush full-width bar) comes
/// entirely from the active [AppNavigationBarTheme]. The decorative background blobs and the
/// [GamepadNotificationBar] are placed here too, so every branch gets them.
///
/// When the nav bar floats (a translucent pill, not [AppNavigationBarTheme.stretchToFullWidth]),
/// content is allowed to run edge-to-edge behind it instead of reserving opaque space for it,
/// so the bar only ever shows page content and blobs through its own blur, never a bare
/// background. Only the flush, opaque Legacy bar reserves real space so it isn't drawn over.
class AppShell extends ConsumerStatefulWidget {
  const AppShell({super.key, required this.navigationShell});

  final StatefulNavigationShell navigationShell;

  @override
  ConsumerState<AppShell> createState() => _AppShellState();
}

class _AppShellState extends ConsumerState<AppShell> {
  final ValueNotifier<double> _navBarOffset = ValueNotifier(0);
  double _hiddenDistance = 0;
  int? _lastBranchIndex;
  late final ShellNavigationController _shellNavigation;

  @override
  void initState() {
    super.initState();
    _shellNavigation = ref.read(shellNavigationProvider);
  }

  @override
  void dispose() {
    _navBarOffset.dispose();
    _shellNavigation.detach(widget.navigationShell);
    super.dispose();
  }

  static const double _standardNavBarHeight = 80;

  Widget _standardNavigationBar(BuildContext context) {
    final destinations = AppNavDestination.of(context);
    return MediaQuery.removePadding(
      context: context,
      removeTop: true,
      child: NavigationBar(
        selectedIndex: widget.navigationShell.currentIndex,
        onDestinationSelected: (index) => widget.navigationShell.goBranch(
          index,
          initialLocation: widget.navigationShell.currentIndex == index,
        ),
        destinations: [
          for (final destination in destinations)
            NavigationDestination(
              icon: Icon(destination.icon),
              label: destination.label,
            ),
        ],
      ),
    );
  }

  bool _handleScrollNotification(ScrollUpdateNotification notification) {
    if (notification.metrics.axis != Axis.vertical ||
        notification.metrics.outOfRange) {
      return false;
    }
    final delta = notification.scrollDelta ?? 0;
    _navBarOffset.value = (_navBarOffset.value + delta).clamp(
      0.0,
      _hiddenDistance,
    );
    return false;
  }

  @override
  Widget build(BuildContext context) {
    _shellNavigation.attach(widget.navigationShell);
    final navBarTheme = Theme.of(context).extension<AppNavigationBarTheme>()!;
    final blobVariant =
        BackgroundBlobVariant.values[widget.navigationShell.currentIndex];
    final reduceMotion = ref.watch(accessibilitySettingsProvider).reduceMotion;
    final animationSpeed = ref.watch(advancedSettingsProvider).animationSpeed;
    final animationDuration = resolveAnimationDuration(
      reduceMotion: reduceMotion,
      speed: animationSpeed,
    );
    if (ref.watch(isDesktopPlatformProvider)) {
      return DesktopAppShell(navigationShell: widget.navigationShell);
    }
    final standardNavBar = ref.watch(
      themeSettingsProvider.select(
        (settings) => settings.themeStyle == ThemeStyle.neumorphic,
      ),
    );
    final reservedHeight = standardNavBar
        ? _standardNavBarHeight
        : navBarTheme.contentHeight + navBarTheme.bottomMargin;
    _hiddenDistance = standardNavBar
        ? _standardNavBarHeight + MediaQuery.paddingOf(context).bottom + 32
        : reservedHeight + 32;
    final branchIndex = widget.navigationShell.currentIndex;
    if (_lastBranchIndex != branchIndex) {
      if (_lastBranchIndex != null) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) _navBarOffset.value = 0;
        });
      }
      _lastBranchIndex = branchIndex;
    }
    return Scaffold(
      body: NotificationListener<ScrollUpdateNotification>(
        onNotification: _handleScrollNotification,
        child: Stack(
          children: [
            Positioned.fill(child: BackgroundBlobs(variant: blobVariant)),
            GamepadNotificationColumn(
              child: Stack(
                children: [
                  _NavBarInsets(
                    bottomInset: reservedHeight,
                    addSystemInset: standardNavBar,
                    child: widget.navigationShell,
                  ),
                  Positioned(
                    left: standardNavBar ? 0 : navBarTheme.horizontalMargin,
                    right: standardNavBar ? 0 : navBarTheme.horizontalMargin,
                    bottom: standardNavBar ? 0 : navBarTheme.bottomMargin,
                    child: ValueListenableBuilder<double>(
                      valueListenable: _navBarOffset,
                      builder: (context, offset, child) => Transform.translate(
                        offset: Offset(0, offset),
                        child: child,
                      ),
                      child: GamepadFocusRegion(
                        child: standardNavBar
                            ? _standardNavigationBar(context)
                            : navBarTheme.stretchToFullWidth
                            ? AppNavBar(
                                navigationShell: widget.navigationShell,
                                animationDuration: animationDuration,
                              )
                            : Center(
                                child: AppNavBar(
                                  navigationShell: widget.navigationShell,
                                  animationDuration: animationDuration,
                                ),
                              ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Sets the ambient [MediaQuery] bottom padding to the height the overlaid nav bar covers from the
/// bottom edge, so scrollable content can pad itself clear of the bar. The standard
/// `NavigationBar` pads itself by the system inset, so [addSystemInset] adds it on top; the other
/// bars sit over the system inset, which then must not be counted twice.
class _NavBarInsets extends StatelessWidget {
  const _NavBarInsets({
    required this.bottomInset,
    required this.addSystemInset,
    required this.child,
  });

  final double bottomInset;
  final bool addSystemInset;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);
    final systemInset = addSystemInset ? mediaQuery.padding.bottom : 0;
    return MediaQuery(
      data: mediaQuery.copyWith(
        padding: mediaQuery.padding.copyWith(bottom: systemInset + bottomInset),
      ),
      child: child,
    );
  }
}
