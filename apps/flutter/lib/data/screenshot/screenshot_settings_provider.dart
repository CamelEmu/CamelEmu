import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app_services.dart';

@immutable
class ScreenshotSettings {
  const ScreenshotSettings({this.directory, this.alwaysAsk = false});

  final String? directory;
  final bool alwaysAsk;

  ScreenshotSettings copyWith({String? directory, bool? alwaysAsk}) {
    return ScreenshotSettings(
      directory: directory ?? this.directory,
      alwaysAsk: alwaysAsk ?? this.alwaysAsk,
    );
  }
}

final screenshotSettingsProvider =
    AsyncNotifierProvider<ScreenshotSettingsNotifier, ScreenshotSettings>(
      ScreenshotSettingsNotifier.new,
    );

class ScreenshotSettingsNotifier extends AsyncNotifier<ScreenshotSettings> {
  @override
  Future<ScreenshotSettings> build() async {
    final repository = AppServices.screenshotSettingsRepository;
    return ScreenshotSettings(
      directory: await repository.directory(),
      alwaysAsk: await repository.alwaysAsk(),
    );
  }

  Future<String?> pickDirectory() => Platform.isAndroid
      ? AppServices.nativeBridge.openScreenshotDirectory()
      : FilePicker.getDirectoryPath();

  Future<void> chooseDirectory() async {
    final picked = await pickDirectory();
    if (picked != null) await setDirectory(picked);
  }

  Future<void> setDirectory(String directory) async {
    final current = await future;
    await AppServices.screenshotSettingsRepository.setDirectory(directory);
    state = AsyncData(current.copyWith(directory: directory));
  }

  Future<void> setAlwaysAsk(bool value) async {
    final current = await future;
    await AppServices.screenshotSettingsRepository.setAlwaysAsk(value);
    state = AsyncData(current.copyWith(alwaysAsk: value));
  }
}
