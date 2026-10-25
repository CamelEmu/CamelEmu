import 'package:flutter/material.dart';

import '../../../data/tags/abstract_tag.dart';
import '../../../widgets/pinned_below_header.dart';
import 'tag_filter_bar.dart';

/// The tag filter of the games list. It is not part of the header: when the header slides out of
/// sight with the scrolling, this stays right below the status bar.
class GameFilterSection extends StatefulWidget {
  const GameFilterSection({
    super.key,
    required this.tags,
    required this.selectedTagIds,
    required this.onToggle,
    required this.onSelectAll,
    this.enabled = true,
  });

  final List<AbstractTag> tags;
  final Set<String> selectedTagIds;
  final ValueChanged<String> onToggle;
  final VoidCallback onSelectAll;
  final bool enabled;

  @override
  State<GameFilterSection> createState() => _GameFilterSectionState();
}

class _GameFilterSectionState extends State<GameFilterSection>
    with PinnedBelowHeader {
  @override
  Widget build(BuildContext context) {
    reportPinnedHeight();
    return TagFilterBar(
      tags: widget.tags,
      selectedTagIds: widget.selectedTagIds,
      onToggle: widget.onToggle,
      onSelectAll: widget.onSelectAll,
      enabled: widget.enabled,
    );
  }
}
