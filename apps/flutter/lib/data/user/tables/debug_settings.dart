part of '../user_database.dart';

class DebugSettings extends Table {
  IntColumn get id => integer().withDefault(const Constant(0))();
  BoolColumn get logToConsole => boolean().withDefault(const Constant(true))();
  BoolColumn get suppressDescriptorWarning =>
      boolean().withDefault(const Constant(false))();

  @override
  Set<Column> get primaryKey => {id};
}
