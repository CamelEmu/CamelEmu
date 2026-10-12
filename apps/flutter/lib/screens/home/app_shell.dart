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
import '../../widgets/app_nav_bar.dart';
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
    _hiddenDistance = navBarTheme.barHeight + navBarTheme.bottomMargin + 32;
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
                  if (navBarTheme.stretchToFullWidth)
                    _NavBarInsets(
                      navBarTheme: navBarTheme,
                      child: widget.navigationShell,
                    )
                  else
                    widget.navigationShell,
                  Positioned(
                    left: navBarTheme.horizontalMargin,
                    right: navBarTheme.horizontalMargin,
                    bottom: navBarTheme.bottomMargin,
                    child: ValueListenableBuilder<double>(
                      valueListenable: _navBarOffset,
                      builder: (context, offset, child) => Transform.translate(
                        offset: Offset(0, offset),
                        child: child,
                      ),
                      child: GamepadFocusRegion(
                        child: navBarTheme.stretchToFullWidth
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

/// Grows the ambient [MediaQuery] bottom padding by the nav bar's height, so `SafeArea`s inside
/// [child] leave room for the flush, opaque bar instead of being covered by it.
class _NavBarInsets extends StatelessWidget {
  const _NavBarInsets({required this.navBarTheme, required this.child});

  final AppNavigationBarTheme navBarTheme;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);
    final extraBottomInset = navBarTheme.barHeight + navBarTheme.bottomMargin;
    return MediaQuery(
      data: mediaQuery.copyWith(
        padding: mediaQuery.padding.copyWith(
          bottom: mediaQuery.padding.bottom + extraBottomInset,
        ),
      ),
      child: child,
    );
  }
}
