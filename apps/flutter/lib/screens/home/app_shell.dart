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
import '../../widgets/app_widgets.dart';
import '../../widgets/background_blobs.dart';
import '../../widgets/gamepad/gamepad_focus_region.dart';
import '../../widgets/gamepad_notification_bar.dart';
import '../../widgets/pinned_below_header.dart';
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
  final ValueNotifier<double> _headerHidden = ValueNotifier(0);
  final GlobalKey _pageHostKey = GlobalKey();
  final PinnedHeaderRegistry _pinned = PinnedHeaderRegistry();
  final GlobalKey _customNavBarKey = GlobalKey();
  double _customNavBarHeight = 0;
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
    _headerHidden.dispose();
    _shellNavigation.detach(widget.navigationShell);
    super.dispose();
  }

  static const double _standardNavBarHeight = 80;

  Widget? _customNavigationBar(BuildContext context) {
    final custom = AppWidgets.of(context).navigationBar(
      destinations: AppNavDestination.of(context),
      selectedIndex: widget.navigationShell.currentIndex,
      onSelected: (index) => widget.navigationShell.goBranch(
        index,
        initialLocation: widget.navigationShell.currentIndex == index,
      ),
    );
    if (custom == null) return null;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final box = _customNavBarKey.currentContext?.findRenderObject();
      if (!mounted || box is! RenderBox || !box.hasSize) return;
      if (box.size.height != _customNavBarHeight) {
        setState(() => _customNavBarHeight = box.size.height);
      }
    });
    return KeyedSubtree(key: _customNavBarKey, child: custom);
  }

  Widget _standardNavigationBar(BuildContext context, Widget? custom) {
    if (custom != null) return custom;
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

  bool _handleScrollNotification(ScrollNotification notification) {
    if (notification.metrics.axis != Axis.vertical) return false;
    if (notification is OverscrollNotification) {
      if (notification.overscroll < 0) {
        _headerHidden.value = (_headerHidden.value + notification.overscroll)
            .clamp(0.0, _headerHidden.value);
      }
      return false;
    }
    if (notification is! ScrollUpdateNotification ||
        notification.metrics.outOfRange) {
      return false;
    }
    final delta = notification.scrollDelta ?? 0;
    _navBarOffset.value = (_navBarOffset.value + delta).clamp(
      0.0,
      _hiddenDistance,
    );
    _slideHeader(notification, delta);
    return false;
  }

  /// Slides the header of the page, which is whatever the page lays out above the scrolling list
  /// apart from its [PinnedBelowHeader] sections, out of sight by [delta], up to its own height. The page is moved up and made taller by the
  /// hidden distance, and the scroll position is corrected by the same distance, so the content
  /// moves together with the header at the speed of the scrolling.
  void _slideHeader(ScrollUpdateNotification notification, double delta) {
    final scrollable = notification.context;
    final host = _pageHostKey.currentContext?.findRenderObject();
    if (scrollable == null || host is! RenderBox) return;
    final list = scrollable.findRenderObject();
    if (list is! RenderBox || !list.attached) return;
    final hidden = _headerHidden.value;
    final listTop = list.localToGlobal(Offset.zero, ancestor: host).dy;
    final statusBar = MediaQuery.paddingOf(context).top;
    final headerHeight = (listTop + hidden - statusBar - _pinned.height).clamp(
      0.0,
      1000.0,
    );
    _headerHidden.value = (hidden + delta).clamp(0.0, headerHeight);
    final change = _headerHidden.value - hidden;
    if (change != 0) {
      Scrollable.maybeOf(scrollable)?.position.correctBy(-change);
    }
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
        (settings) =>
            settings.themeStyle == ThemeStyle.neumorphic ||
            settings.themeStyle == ThemeStyle.shadcn,
      ),
    );
    final customNavBar = standardNavBar ? _customNavigationBar(context) : null;
    final systemInset = MediaQuery.paddingOf(context).bottom;
    final reservedHeight = customNavBar != null
        ? (_customNavBarHeight - systemInset).clamp(0.0, double.infinity)
        : standardNavBar
        ? _standardNavBarHeight
        : navBarTheme.contentHeight + navBarTheme.bottomMargin;
    _hiddenDistance = standardNavBar
        ? reservedHeight + systemInset
        : reservedHeight + 32;
    final branchIndex = widget.navigationShell.currentIndex;
    if (_lastBranchIndex != branchIndex) {
      if (_lastBranchIndex != null) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (!mounted) return;
          _navBarOffset.value = 0;
          _headerHidden.value = 0;
        });
      }
      _lastBranchIndex = branchIndex;
    }
    return Scaffold(
      body: NotificationListener<ScrollNotification>(
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
                    child: PinnedHeaderScope(
                      registry: _pinned,
                      child: _PageHost(
                        key: _pageHostKey,
                        hidden: _headerHidden,
                        child: widget.navigationShell,
                      ),
                    ),
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
                            ? _standardNavigationBar(context, customNavBar)
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

/// Shows [child] moved up by [hidden] and made taller by the same distance, so that what it lays
/// out at the top slides out of sight while the bottom stays where it is.
class _PageHost extends StatelessWidget {
  const _PageHost({super.key, required this.hidden, required this.child});

  final ValueNotifier<double> hidden;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) => ClipRect(
        clipper: _BelowStatusBarClipper(MediaQuery.paddingOf(context).top),
        child: ValueListenableBuilder<double>(
          valueListenable: hidden,
          child: child,
          builder: (context, hidden, child) => Stack(
            clipBehavior: Clip.none,
            children: [
              Positioned(
                left: 0,
                right: 0,
                top: -hidden,
                height: constraints.maxHeight + hidden,
                child: child!,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Clips away everything above the status bar inset, so that a header slid up never shows in the
/// status bar area.
class _BelowStatusBarClipper extends CustomClipper<Rect> {
  const _BelowStatusBarClipper(this.top);

  final double top;

  @override
  Rect getClip(Size size) => Rect.fromLTRB(0, top, size.width, size.height);

  @override
  bool shouldReclip(_BelowStatusBarClipper oldClipper) => oldClipper.top != top;
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
