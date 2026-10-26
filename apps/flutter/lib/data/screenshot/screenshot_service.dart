import 'dart:io';

import 'package:azahar_for_flutter/azahar_for_flutter.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app_services.dart';
import 'screenshot_settings_provider.dart';

enum ScreenshotStatus { saved, cancelled, failed }

typedef ScreenshotOutcome = ({ScreenshotStatus status, String? fileName});

final screenshotServiceProvider = Provider<ScreenshotService>(
  ScreenshotService.new,
);

class ScreenshotService {
  ScreenshotService(this._ref);

  final Ref _ref;

  ScreenshotSettingsNotifier get _settings =>
      _ref.read(screenshotSettingsProvider.notifier);

  Future<CaptureResult?> take(CaptureKind kind, CaptureTarget target) async {
    try {
      return await capture(kind: kind, target: target);
    } on Object {
      return null;
    }
  }

  Future<ScreenshotOutcome> save(CaptureResult? result) async {
    if (result == null) {
      return (status: ScreenshotStatus.failed, fileName: null);
    }

    final settings = await _ref.read(screenshotSettingsProvider.future);
    var directory = settings.directory;
    if (settings.alwaysAsk || directory == null) {
      final picked = await _settings.pickDirectory();
      if (picked == null) {
        return (status: ScreenshotStatus.cancelled, fileName: null);
      }
      if (!settings.alwaysAsk) await _settings.setDirectory(picked);
      directory = picked;
    }

    final written = await _write(directory, result);
    return (
      status: written ? ScreenshotStatus.saved : ScreenshotStatus.failed,
      fileName: result.fileName,
    );
  }

  Future<bool> _write(String directory, CaptureResult result) async {
    if (Platform.isAndroid) {
      return AppServices.nativeBridge.writeToDirectory(
        treeUri: directory,
        fileName: result.fileName,
        mimeType: result.mimeType,
        bytes: result.bytes,
      );
    }
    try {
      await File(
        '$directory${Platform.pathSeparator}${result.fileName}',
      ).writeAsBytes(result.bytes);
      return true;
    } on FileSystemException {
      return false;
    }
  }
}
