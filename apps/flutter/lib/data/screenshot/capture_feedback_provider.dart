import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'screenshot_service.dart';

enum CaptureFeedback { idle, working, succeeded, failed }

final captureFeedbackProvider =
    NotifierProvider<CaptureFeedbackNotifier, CaptureFeedback>(
      CaptureFeedbackNotifier.new,
    );

class CaptureFeedbackNotifier extends Notifier<CaptureFeedback> {
  static const _resultDuration = Duration(milliseconds: 1200);

  bool _cancelled = false;

  bool get isCancelled => _cancelled;

  @override
  CaptureFeedback build() => CaptureFeedback.idle;

  bool begin() {
    if (state != CaptureFeedback.idle) return false;
    _cancelled = false;
    state = CaptureFeedback.working;
    return true;
  }

  void cancel() {
    if (state != CaptureFeedback.working) return;
    _cancelled = true;
    state = CaptureFeedback.idle;
  }

  Future<void> finish(ScreenshotStatus status) async {
    if (_cancelled) return;
    final result = switch (status) {
      ScreenshotStatus.saved => CaptureFeedback.succeeded,
      ScreenshotStatus.failed => CaptureFeedback.failed,
      ScreenshotStatus.cancelled => CaptureFeedback.idle,
    };
    state = result;
    if (result == CaptureFeedback.idle) return;
    await Future<void>.delayed(_resultDuration);
    if (state == result) state = CaptureFeedback.idle;
  }
}
