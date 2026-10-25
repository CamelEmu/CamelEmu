import 'package:flutter/material.dart';

import '../theme/extensions/background_blob_theme.dart';

/// Paints the decorative gradient blobs behind a screen. Renders nothing under the
/// Legacy theme, since its [BackgroundBlobTheme] carries no blobs.
class BackgroundBlobs extends StatelessWidget {
  const BackgroundBlobs({super.key, required this.variant});

  final BackgroundBlobVariant variant;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context).extension<BackgroundBlobTheme>()!;
    final blobs = theme.blobsByVariant[variant] ?? const [];
    if (blobs.isEmpty) return const SizedBox.shrink();
    final stack = Stack(
      children: [
        for (final blob in blobs)
          Align(
            alignment: blob.alignment,
            child: Container(
              width: blob.size * 1.5,
              height: blob.size * 1.5,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                gradient: RadialGradient(
                  colors: [blob.color, blob.color.withValues(alpha: 0)],
                ),
              ),
            ),
          ),
      ],
    );
    return IgnorePointer(
      child: ClipRect(child: RepaintBoundary(child: stack)),
    );
  }
}
