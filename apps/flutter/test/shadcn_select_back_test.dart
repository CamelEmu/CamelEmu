import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shadcn;
import 'package:shadcn_flutter_material/shadcn_flutter_material.dart'
    as shadcn_material;

import 'package:azahar/widgets/shadcn_widget_set.dart';

GoRouter _router(List<String> selected) {
  return GoRouter(
    routes: [
      GoRoute(
        path: '/',
        builder: (context, state) => Scaffold(
          body: Center(
            child: ElevatedButton(
              onPressed: () => context.push('/options'),
              child: const Text('open options'),
            ),
          ),
        ),
      ),
      GoRoute(
        path: '/options',
        builder: (context, state) => Scaffold(
          appBar: AppBar(title: const Text('options page')),
          body: Builder(
            builder: (context) =>
                const ShadcnWidgetSet().select<String>(
                  labels: const ['Real', 'Fixed'],
                  values: const ['real', 'fixed'],
                  value: 'real',
                  onChanged: selected.add,
                ) ??
                const SizedBox.shrink(),
          ),
        ),
      ),
    ],
  );
}

Widget _app(List<String> selected) {
  return shadcn_material.MaterialShadcnApp.router(
    routerConfig: _router(selected),
    scaffoldMessenger: false,
    localizationsDelegates: const [DefaultMaterialLocalizations.delegate],
    theme: shadcn.ThemeData(colorScheme: shadcn.ColorSchemes.lightZinc),
    builder: (context, child) =>
        ScaffoldMessenger(child: child ?? const SizedBox.shrink()),
  );
}

Future<void> _openDropdown(WidgetTester tester, List<String> selected) async {
  await tester.pumpWidget(_app(selected));
  await tester.pumpAndSettle();
  await tester.tap(find.text('open options'));
  await tester.pumpAndSettle();
  expect(find.text('options page'), findsOneWidget);
  await tester.tap(find.text('Real'));
  await tester.pumpAndSettle();
  expect(find.text('Fixed'), findsOneWidget);
}

void main() {
  group('the dropdown of an enum option under the Shadcn app', () {
    testWidgets('closes on back without leaving the page', (tester) async {
      final selected = <String>[];
      await _openDropdown(tester, selected);

      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();

      expect(find.text('Fixed'), findsNothing);
      expect(find.text('options page'), findsOneWidget);
      expect(selected, isEmpty);
    });

    testWidgets('leaves the page on the next back', (tester) async {
      final selected = <String>[];
      await _openDropdown(tester, selected);
      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();

      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();

      expect(find.text('Fixed'), findsNothing);
      expect(find.text('options page'), findsNothing);
      expect(find.text('open options'), findsOneWidget);
    });

    testWidgets('reports the picked choice', (tester) async {
      final selected = <String>[];
      await _openDropdown(tester, selected);

      await tester.tap(find.text('Fixed'));
      await tester.pumpAndSettle();

      expect(selected, ['fixed']);
      expect(find.text('options page'), findsOneWidget);
    });
  });
}
