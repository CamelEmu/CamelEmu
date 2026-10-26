import 'dart:async';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:azahar_for_flutter/azahar_for_flutter.dart';

import '../../app_services.dart';
import '../../data/gamepad/gamepad_hub.dart';
import '../../data/platform_provider.dart';
import '../../data/repositories/cheat_repository.dart';
import '../../data/screenshot/capture_feedback_provider.dart';
import '../../data/screenshot/screenshot_service.dart';
import '../../data/settings/country.dart';
import '../../theme/extensions/emulation_focus_frame_theme.dart';
import 'controller_motion_source.dart';
import 'emulation_session_state.dart';
import 'emulation_focus_provider.dart';
import 'emulation_screens_layout.dart';
import 'emulation_session_provider.dart';
import 'motion_input_source.dart';
import 'virtual_gamepad_visibility_provider.dart';
import 'widgets/bottom_screen.dart';
import 'widgets/artic_country_mismatch_dialog.dart';
import 'widgets/capture_feedback_overlay.dart';
import 'widgets/cheats_dialog.dart';
import 'widgets/emulation_failure_dialog.dart';
import 'widgets/emulation_drawer.dart';
import 'widgets/gamepad/emulation_gamepad.dart';
import 'widgets/emulation_loading_card.dart';
import 'widgets/emulation_menu_actions.dart';
import 'widgets/emulation_side_panel.dart';
import 'widgets/exit_without_saving_dialog.dart';
import 'widgets/game_ended_dialog.dart';
import 'widgets/top_screen.dart';

class EmulationPage extends ConsumerStatefulWidget {
  const EmulationPage({
    super.key,
    required this.gamePath,
    required this.ready,
    this.game,
  });

  final String gamePath;
  final Game? game;
  final Future<void> ready;

  @override
  ConsumerState<EmulationPage> createState() => _EmulationPageState();
}

class _EmulationPageState extends ConsumerState<EmulationPage>
    with WidgetsBindingObserver {
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();
  bool _launchRequested = false;
  late final EmulationFocusNotifier _focusNotifier;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _focusNotifier = ref.read(emulationFocusProvider.notifier);
    Future.microtask(() => _focusNotifier.activate(this));
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    Future.microtask(() => _focusNotifier.deactivate(this));
    super.dispose();
  }

  void _onEmulationFocusChanged(EmulationFocus focus) {
    final scaffold = _scaffoldKey.currentState;
    if (scaffold == null || ref.read(isDesktopPlatformProvider)) return;
    switch (focus) {
      case EmulationFocus.menu:
        final isCapturing =
            ref.read(captureFeedbackProvider) != CaptureFeedback.idle;
        if (!scaffold.isDrawerOpen && !isCapturing) scaffold.openDrawer();
      case EmulationFocus.game:
        if (scaffold.isDrawerOpen) scaffold.closeDrawer();
    }
  }

  void _onDrawerChanged(bool isOpened) {
    _focusNotifier.focus(isOpened ? EmulationFocus.menu : EmulationFocus.game);
  }

  @override
  Future<AppExitResponse> didRequestAppExit() async {
    await ref.read(emulationSessionProvider.notifier).stopForExit();
    return AppExitResponse.exit;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    final notifier = ref.read(emulationSessionProvider.notifier);
    switch (state) {
      case AppLifecycleState.paused:
      case AppLifecycleState.hidden:
        notifier.handleAppBackground();
      case AppLifecycleState.resumed:
        notifier.handleAppForeground();
      case AppLifecycleState.inactive:
      case AppLifecycleState.detached:
        break;
    }
  }

  void _requestLaunch(EmulationScreensLayout layout) {
    if (_launchRequested) return;
    if (layout.topScreen.isEmpty || layout.bottomScreen.isEmpty) return;
    _launchRequested = true;
    final devicePixelRatio = MediaQuery.devicePixelRatioOf(context);
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await widget.ready;
      if (!mounted) return;
      await ref
          .read(emulationSessionProvider.notifier)
          .launch(
            gamePath: widget.gamePath,
            layout: layout,
            devicePixelRatio: devicePixelRatio,
          );
    });
  }

  void _requestMediaSessionActivation() {
    final game = widget.game;
    if (game == null) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref
          .read(emulationSessionProvider.notifier)
          .activateMediaSessionIfNeeded(game);
    });
  }

  void _handleBackPressed() {
    if (ref.read(captureFeedbackProvider) != CaptureFeedback.idle) {
      ref.read(captureFeedbackProvider.notifier).cancel();
      return;
    }
    final state = ref.read(emulationSessionProvider);
    if (!state.emulationStarted || state.isTerminating) return;
    final scaffold = _scaffoldKey.currentState;
    if (scaffold == null) return;
    if (scaffold.isDrawerOpen) {
      scaffold.closeDrawer();
    } else {
      scaffold.openDrawer();
    }
  }

  Future<void> _saveAndExit() {
    return ref.read(emulationSessionProvider.notifier).terminate(save: true);
  }

  Future<void> _confirmExitWithoutSaving() async {
    final notifier = ref.read(emulationSessionProvider.notifier);
    final wasRunning = await notifier.pauseForDialog();
    if (!mounted) return;
    final confirmed = await ExitWithoutSavingDialog.show(context);
    if (!mounted) return;
    if (confirmed == true) {
      await notifier.terminate(save: false);
    } else {
      await notifier.resumeAfterDialog(wasRunning: wasRunning);
    }
  }

  /// Lets the user choose to save or discard the changes once the game itself asked to end,
  /// instead of closing the screen at once.
  Future<void> _askToSaveAfterGameEnded() async {
    final notifier = ref.read(emulationSessionProvider.notifier);
    _scaffoldKey.currentState?.closeDrawer();
    final save = await GameEndedDialog.show(context);
    await notifier.terminate(save: save);
  }

  Future<void> _showFailure(String message) async {
    final notifier = ref.read(emulationSessionProvider.notifier);
    _scaffoldKey.currentState?.closeDrawer();
    await EmulationFailureDialog.show(context, message);
    await notifier.terminate(save: true);
  }

  Future<void> _askToOverrideCountry(ArticCountryMismatch mismatch) async {
    final notifier = ref.read(emulationSessionProvider.notifier);
    _scaffoldKey.currentState?.closeDrawer();
    final country = Country.defaultForRegion(mismatch.region);
    final override = await ArticCountryMismatchDialog.show(
      context,
      region: mismatch.region,
      current: Country.fromCode(mismatch.country),
      replacement: country,
    );
    await notifier.answerArticCountry(override ? country.code : null);
    if (!override) await notifier.terminate(save: true);
  }

  Future<void> _openCheats() async {
    final game = widget.game;
    if (game == null) return;
    final notifier = ref.read(emulationSessionProvider.notifier);
    final wasRunning = await notifier.pauseForDialog();
    try {
      if (!mounted) return;
      await CheatsDialog.show(
        context,
        repository: CheatRepository(AppServices.nativeBridge, game.titleId),
      );
    } finally {
      await notifier.resumeAfterDialog(wasRunning: wasRunning);
    }
  }

  Future<void> _capture(CaptureKind kind, CaptureTarget target) async {
    final feedback = ref.read(captureFeedbackProvider.notifier);
    if (!feedback.begin()) return;
    _scaffoldKey.currentState?.closeDrawer();
    final service = ref.read(screenshotServiceProvider);
    final captured = await ref
        .read(emulationSessionProvider.notifier)
        .completeWhilePaused(
          () => service.take(kind, target),
          isCancelled: () => feedback.isCancelled,
        );
    if (!mounted || feedback.isCancelled) return;
    final outcome = await service.save(captured);
    if (!mounted) return;
    await feedback.finish(outcome.status);
  }

  Widget _screens(BoxConstraints constraints) {
    final isDesktop = ref.watch(isDesktopPlatformProvider);
    final layout = EmulationScreensLayout.fit(
      constraints.biggest,
      isDesktop: isDesktop,
    );
    _requestLaunch(layout);
    _requestMediaSessionActivation();
    final notifier = ref.read(emulationSessionProvider.notifier);
    final state = ref.watch(emulationSessionProvider);
    final topScreen = TopScreen(
      textureId: state.topTextureId,
      size: layout.topScreen,
    );
    final bottomScreen = BottomScreen(
      textureId: state.bottomTextureId,
      size: layout.bottomScreen,
      onPointerDown: (event) =>
          notifier.touchPressed(event.localPosition, layout.bottomScreen),
      onPointerMove: (event) =>
          notifier.touchMoved(event.localPosition, layout.bottomScreen),
      onPointerUp: (_) => notifier.touchReleased(),
    );
    final frameTheme = Theme.of(context).extension<EmulationFocusFrameTheme>()!;
    final isFrameHighlighted =
        ref.watch(emulationFocusProvider).isGameFocused &&
        ref.watch(gamepadInputModeProvider) == GamepadInputMode.controller;
    return Align(
      alignment: Alignment.topCenter,
      child: Container(
        foregroundDecoration: isFrameHighlighted
            ? BoxDecoration(
                border: Border.all(
                  color: frameTheme.focusedColor,
                  width: frameTheme.width,
                ),
                borderRadius: frameTheme.borderRadius,
              )
            : null,
        child: Flex(
          direction: layout.direction,
          mainAxisSize: MainAxisSize.min,
          children: state.isScreensSwapped
              ? [bottomScreen, topScreen]
              : [topScreen, bottomScreen],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(emulationSessionProvider.select((state) => state.isFinished), (
      _,
      isFinished,
    ) {
      if (isFinished) Navigator.of(context).pop();
    });
    ref.listen(
      emulationSessionProvider.select((state) => state.articCountryMismatch),
      (_, mismatch) {
        if (mismatch != null) unawaited(_askToOverrideCountry(mismatch));
      },
    );
    ref.listen(emulationSessionProvider.select((state) => state.failure), (
      _,
      failure,
    ) {
      if (failure != null) unawaited(_showFailure(failure));
    });
    ref.listen(
      emulationFocusProvider.select((state) => state.focus),
      (_, focus) => _onEmulationFocusChanged(focus),
    );
    ref.listen(
      emulationSessionProvider.select((state) => state.isShutdownRequested),
      (_, isShutdownRequested) {
        if (isShutdownRequested) unawaited(_askToSaveAfterGameEnded());
      },
    );
    final state = ref.watch(emulationSessionProvider);
    if (state.isClosingWindow) {
      return const ColoredBox(color: Colors.black);
    }
    final isDesktop = ref.watch(isDesktopPlatformProvider);
    final isVirtualGamepadVisible = ref.watch(virtualGamepadVisibleProvider);
    final notifier = ref.read(emulationSessionProvider.notifier);
    final actions = EmulationMenuActions(
      onTogglePause: notifier.togglePause,
      onAdvanceFrame: notifier.advanceFrame,
      onCheats: widget.game == null ? null : _openCheats,
      onCapture: (kind, target) => unawaited(_capture(kind, target)),
      onSaveAndExit: _saveAndExit,
      onExitWithoutSaving: _confirmExitWithoutSaving,
      isVirtualGamepadVisible: isDesktop ? null : isVirtualGamepadVisible,
      onToggleVirtualGamepad: ref
          .read(virtualGamepadVisibleProvider.notifier)
          .toggle,
    );
    final screens = SafeArea(
      child: LayoutBuilder(
        builder: (context, constraints) => _screens(constraints),
      ),
    );
    final page = PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) {
        if (didPop) return;
        _handleBackPressed();
      },
      child: Scaffold(
        key: _scaffoldKey,
        backgroundColor: Colors.black,
        drawerEnableOpenDragGesture: false,
        onDrawerChanged: _onDrawerChanged,
        drawer: !isDesktop && state.emulationStarted
            ? EmulationDrawer(
                gamePath: widget.gamePath,
                game: widget.game,
                isPaused: state.isPaused,
                actions: actions,
              )
            : null,
        body: isDesktop && state.emulationStarted
            ? Stack(
                children: [
                  screens,
                  Positioned(
                    top: 0,
                    bottom: 0,
                    left: 0,
                    child: EmulationSidePanel(
                      gamePath: widget.gamePath,
                      game: widget.game,
                      isPaused: state.isPaused,
                      actions: actions,
                    ),
                  ),
                ],
              )
            : !isDesktop && state.emulationStarted && isVirtualGamepadVisible
            ? Stack(
                children: [
                  screens,
                  const Positioned.fill(child: EmulationGamepad()),
                ],
              )
            : screens,
      ),
    );
    final showLoadingOverlay = !state.emulationStarted || state.isTerminating;
    final captureFeedback = ref.watch(captureFeedbackProvider);
    final content = Stack(
      children: [
        page,
        if (captureFeedback != CaptureFeedback.idle)
          Positioned.fill(
            child: CaptureFeedbackOverlay(feedback: captureFeedback),
          ),
        if (showLoadingOverlay)
          Positioned.fill(
            child: AbsorbPointer(
              child: ColoredBox(
                color: Colors.black,
                child: SafeArea(
                  child: Center(
                    child: EmulationLoadingCard(
                      gamePath: widget.gamePath,
                      game: widget.game,
                      progress: state.shaderProgress,
                      isTerminating: state.isTerminating,
                    ),
                  ),
                ),
              ),
            ),
          ),
      ],
    );
    final dismissible = Actions(
      actions: {
        DismissIntent: CallbackAction<DismissIntent>(
          onInvoke: (_) => _focusNotifier.focus(EmulationFocus.game),
        ),
      },
      child: content,
    );
    final withControllerMotion = ControllerMotionSource(child: dismissible);
    return isDesktop
        ? withControllerMotion
        : MotionInputSource(child: withControllerMotion);
  }
}
