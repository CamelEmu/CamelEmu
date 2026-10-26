import 'package:azahar_for_flutter/azahar_for_flutter.dart';
import 'package:flutter/material.dart';

import '../../../i18n/translations.g.dart';
import 'emulation_menu_item.dart';

class CaptureMenuItem extends StatefulWidget {
  const CaptureMenuItem({
    super.key,
    required this.icon,
    required this.title,
    required this.onCapture,
    this.iconsOnly = false,
  });

  final IconData icon;
  final String title;
  final ValueChanged<CaptureTarget> onCapture;
  final bool iconsOnly;

  @override
  State<CaptureMenuItem> createState() => _CaptureMenuItemState();
}

class _CaptureMenuItemState extends State<CaptureMenuItem> {
  final _controller = OverlayPortalController();
  final _link = LayerLink();
  Size _itemSize = Size.zero;

  void _showChooser() {
    _itemSize = context.size ?? Size.zero;
    _controller.show();
  }

  void _select(CaptureTarget target) {
    _controller.hide();
    widget.onCapture(target);
  }

  @override
  Widget build(BuildContext context) {
    return CompositedTransformTarget(
      link: _link,
      child: OverlayPortal(
        controller: _controller,
        overlayChildBuilder: _buildChooser,
        child: EmulationMenuItem(
          icon: widget.icon,
          title: widget.title,
          iconsOnly: widget.iconsOnly,
          onTap: () => widget.onCapture(CaptureTarget.both),
          onLongPress: _showChooser,
        ),
      ),
    );
  }

  Widget _buildChooser(BuildContext context) {
    final t = context.t;
    final choices = [
      (CaptureTarget.top, Icons.vertical_align_top, t.emulation.captureTop),
      (CaptureTarget.both, widget.icon, widget.title),
      (
        CaptureTarget.bottom,
        Icons.vertical_align_bottom,
        t.emulation.captureBottom,
      ),
    ];
    return Stack(
      children: [
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: _controller.hide,
          ),
        ),
        CompositedTransformFollower(
          link: _link,
          targetAnchor: Alignment.topLeft,
          followerAnchor: Alignment.topLeft,
          offset: Offset(0, -_itemSize.height),
          child: Align(
            alignment: Alignment.topLeft,
            child: Material(
              elevation: 4,
              borderRadius: BorderRadius.circular(8),
              clipBehavior: Clip.antiAlias,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final (target, icon, label) in choices)
                    SizedBox(
                      width: _itemSize.width,
                      height: _itemSize.height,
                      child: EmulationMenuItem(
                        icon: icon,
                        title: label,
                        iconsOnly: widget.iconsOnly,
                        onTap: () => _select(target),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}
