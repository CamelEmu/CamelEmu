import 'bool_option_tile.dart';

/// The widget a [BoolOption] turns into. It behaves exactly like the [BoolOptionTile] it
/// inherits from.
class BoolOptionWidget extends BoolOptionTile {
  const BoolOptionWidget({
    super.key,
    required super.option,
    super.enabled,
    required super.onAccessed,
    required super.onLongPress,
  });
}
