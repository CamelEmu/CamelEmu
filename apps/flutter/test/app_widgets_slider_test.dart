import 'package:azahar/widgets/app_widgets.dart';
import 'package:azahar/widgets/shadcn_widget_set.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shadcn;
import 'package:shadcn_flutter_material/shadcn_flutter_material.dart'
    as shadcn_material;

AppSlider _slider(List<double> changes) {
  return AppSlider(
    value: 5,
    min: 0,
    max: 10,
    divisions: 10,
    onChanged: changes.add,
  );
}

void main() {
  group('AppSlider', () {
    testWidgets('is a Material slider without a widget set', (tester) async {
      await tester.pumpWidget(MaterialApp(home: Scaffold(body: _slider([]))));

      expect(find.byType(Slider), findsOneWidget);
      expect(find.byType(shadcn.Slider), findsNothing);
    });

    testWidgets('is a shadcn slider with the Shadcn widget set', (
      tester,
    ) async {
      await tester.pumpWidget(
        shadcn_material.MaterialShadcnApp(
          theme: shadcn.ThemeData(colorScheme: shadcn.ColorSchemes.lightZinc),
          home: AppWidgets(
            set: const ShadcnWidgetSet(),
            child: Scaffold(body: Center(child: _slider([]))),
          ),
        ),
      );

      expect(find.byType(shadcn.Slider), findsOneWidget);
      expect(find.byType(Slider), findsNothing);
    });

    testWidgets('reports the value dragged to with the Shadcn widget set', (
      tester,
    ) async {
      final changes = <double>[];
      await tester.pumpWidget(
        shadcn_material.MaterialShadcnApp(
          theme: shadcn.ThemeData(colorScheme: shadcn.ColorSchemes.lightZinc),
          home: AppWidgets(
            set: const ShadcnWidgetSet(),
            child: Scaffold(
              body: Center(
                child: SizedBox(width: 300, child: _slider(changes)),
              ),
            ),
          ),
        ),
      );

      await tester.drag(find.byType(shadcn.Slider), const Offset(100, 0));
      await tester.pumpAndSettle();

      expect(changes, isNotEmpty);
      expect(changes.last, greaterThan(5));
    });
  });
}
