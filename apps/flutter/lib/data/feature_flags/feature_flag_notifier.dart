import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app_services.dart';
import '../settings/feature_flags_provider.dart';
import '../settings/settings_load_provider.dart';
import '../user/user_database.dart';
import 'feature_flag.dart';

/// The base of the notifier behind a feature flag. Its state is whether the flag is on, which
/// [value] exposes, and [switchTo] turns it on or off and keeps the choice under the flag [id].
/// The [FeatureFlag] listed under [id] decides the value a flag has until it is switched, and
/// whether it can be switched at all.
abstract class FeatureFlagNotifier extends Notifier<bool> {
  String get id;

  bool get value => state;

  FeatureFlag get _flag =>
      ref.read(featureFlagsProvider).firstWhere((flag) => flag.id == id);

  @override
  bool build() {
    ref.watch(settingsLoadProvider);
    final flag = _flag;
    if (!flag.editable) return flag.defaultValue;
    return AppServices.featureFlagsRepository.valueOf(
      id,
      defaultValue: flag.defaultValue,
    );
  }

  Future<void> switchTo(bool value) async {
    if (!_flag.editable) return;
    state = value;
    await AppServices.featureFlagsRepository.write(
      FeatureFlagSetting(id: id, value: value),
    );
  }
}
