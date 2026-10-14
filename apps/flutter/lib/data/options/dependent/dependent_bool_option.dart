part of '../abstract_base_option.dart';

/// A [BoolOption] that can only be switched while its [dependencies] are satisfied.
class DependentBoolOption extends AbstractDependentOption<bool> {
  const DependentBoolOption({
    required super.title,
    super.description,
    required super.icon,
    required super.value,
    required this.dependencies,
  });

  @override
  final List<OptionDependency> dependencies;

  @override
  Widget toWidget({
    required VoidCallback onAccessed,
    required VoidCallback onLongPress,
  }) => BoolOptionWidget(
    option: BoolOption(
      title: title,
      description: description,
      icon: icon,
      value: value,
    ),
    enabled: isEnabled,
    onAccessed: onAccessed,
    onLongPress: onLongPress,
  );
}
