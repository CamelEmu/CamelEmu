import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'option_value.dart';

/// Something an option depends on. The option can only be used while [isSatisfied] is true.
abstract class OptionDependency {
  const OptionDependency();

  bool isSatisfied(WidgetRef ref);
}

/// Requires the on/off value [value] to be on.
class BoolOptionDependency extends OptionDependency {
  const BoolOptionDependency(this.value);

  final OptionValue<bool> value;

  @override
  bool isSatisfied(WidgetRef ref) => value.read(ref);
}
