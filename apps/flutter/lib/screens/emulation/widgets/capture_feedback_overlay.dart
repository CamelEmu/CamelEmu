import 'package:flutter/material.dart';

import '../../../data/screenshot/capture_feedback_provider.dart';
import '../../../widgets/app_widgets.dart';

class CaptureFeedbackOverlay extends StatelessWidget {
  const CaptureFeedbackOverlay({super.key, required this.feedback});

  static const _extent = 72.0;
  static const _indicatorExtent = 32.0;
  static const _radius = 16.0;

  final CaptureFeedback feedback;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final child = switch (feedback) {
      CaptureFeedback.working => const AppCircularProgress(
        key: ValueKey('working'),
        size: _indicatorExtent,
      ),
      CaptureFeedback.succeeded => Icon(
        Icons.check,
        key: const ValueKey('succeeded'),
        size: _indicatorExtent,
        color: colors.primary,
      ),
      CaptureFeedback.failed => Icon(
        Icons.error_outline,
        key: const ValueKey('failed'),
        size: _indicatorExtent,
        color: colors.error,
      ),
      CaptureFeedback.idle => const SizedBox.shrink(key: ValueKey('idle')),
    };
    return AbsorbPointer(
      child: Center(
        child: Container(
          width: _extent,
          height: _extent,
          decoration: BoxDecoration(
            color: colors.surfaceContainerHigh,
            borderRadius: BorderRadius.circular(_radius),
          ),
          alignment: Alignment.center,
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 200),
            child: child,
          ),
        ),
      ),
    );
  }
}
