part of '../abstract_base_option.dart';

/// An option that can only be used while everything in its [dependencies] is satisfied. Each kind
/// of option decides what it depends on.
sealed class AbstractDependentOption<T> extends AbstractBaseOption
    with OptionReadable<T>, OptionApplicable<T> {
  const AbstractDependentOption({
    required this.title,
    this.description,
    required this.icon,
    required this.value,
  });

  @override
  final TranslationText title;

  @override
  final TranslationText? description;

  @override
  final IconData icon;

  final OptionValue<T> value;

  List<OptionDependency> get dependencies;

  bool isEnabled(WidgetRef ref) =>
      dependencies.every((dependency) => dependency.isSatisfied(ref));

  @override
  T read(WidgetRef ref) => value.read(ref);

  @override
  Future<void> apply(BuildContext context, WidgetRef ref, T value) =>
      this.value.write(context, ref, value);
}
