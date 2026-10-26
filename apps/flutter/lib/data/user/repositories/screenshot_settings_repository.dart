import 'key_value_repository.dart';

class ScreenshotSettingsRepository extends KeyValueRepository {
  ScreenshotSettingsRepository(super.db);

  static const _directoryKey = 'screenshot_directory';
  static const _alwaysAskKey = 'screenshot_always_ask';

  Future<String?> directory() => read(_directoryKey);

  Future<void> setDirectory(String uri) => write(_directoryKey, uri);

  Future<bool> alwaysAsk() => readBool(_alwaysAskKey);

  Future<void> setAlwaysAsk(bool value) => writeBool(_alwaysAskKey, value);
}
