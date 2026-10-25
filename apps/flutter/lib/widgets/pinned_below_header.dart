import 'package:flutter/material.dart';

/// The heights of the sections of a page that are not part of its header, which the shell must not
/// slide out of sight with it. Only the sections that are shown count, because the pages of the
/// other tabs stay mounted offstage.
class PinnedHeaderRegistry {
  final Map<Object, ({double height, bool Function() isShown})> _sections = {};

  double get height => _sections.values
      .where((section) => section.isShown())
      .fold(0, (sum, section) => sum + section.height);

  void report(Object owner, double height, bool Function() isShown) {
    _sections[owner] = (height: height, isShown: isShown);
  }

  void remove(Object owner) => _sections.remove(owner);
}

class PinnedHeaderScope extends InheritedWidget {
  const PinnedHeaderScope({
    super.key,
    required this.registry,
    required super.child,
  });

  final PinnedHeaderRegistry registry;

  static PinnedHeaderRegistry? maybeOf(BuildContext context) {
    return context.getInheritedWidgetOfExactType<PinnedHeaderScope>()?.registry;
  }

  @override
  bool updateShouldNotify(PinnedHeaderScope oldWidget) => false;
}

/// Makes a section of a page stay right below the status bar when the header above it slides out
/// of sight with the scrolling. The section keeps taking its own room in the layout, and the shell
/// slides only the part above it. Call [reportPinnedHeight] from `build`.
mixin PinnedBelowHeader<T extends StatefulWidget> on State<T> {
  PinnedHeaderRegistry? _registry;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _registry = PinnedHeaderScope.maybeOf(context);
  }

  void reportPinnedHeight() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final box = context.findRenderObject();
      if (box is RenderBox && box.hasSize) {
        _registry?.report(this, box.size.height, _isShown);
      }
    });
  }

  bool _isShown() {
    if (!mounted) return false;
    var shown = true;
    context.visitAncestorElements((element) {
      final widget = element.widget;
      if (widget is Offstage && widget.offstage) shown = false;
      return shown;
    });
    return shown;
  }

  @override
  void dispose() {
    _registry?.remove(this);
    super.dispose();
  }
}
