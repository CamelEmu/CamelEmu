import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../i18n/translations.g.dart';
import '../../../widgets/app_widgets.dart';

class MessageDialog extends StatelessWidget {
  const MessageDialog({
    super.key,
    required this.title,
    this.description,
    this.helpUrl,
  });

  final String title;
  final String? description;
  final String? helpUrl;

  static Future<void> show(
    BuildContext context, {
    required String title,
    String? description,
    String? helpUrl,
  }) {
    return showDialog<void>(
      context: context,
      builder: (_) => MessageDialog(
        title: title,
        description: description,
        helpUrl: helpUrl,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = context.t;
    return AlertDialog(
      title: Text(title),
      content: description != null ? Text(description!) : null,
      actions: [
        if (helpUrl != null)
          AppTextButton(
            onPressed: () => launchUrl(
              Uri.parse(helpUrl!),
              mode: LaunchMode.externalApplication,
            ),
            child: Text(t.setup.warningHelp),
          ),
        AppTextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(t.setup.close),
        ),
      ],
    );
  }
}
