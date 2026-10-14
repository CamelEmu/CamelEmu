import 'dart:io';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_crashlytics/firebase_crashlytics.dart';
import 'package:flutter/foundation.dart';

/// Sends the errors the app does not handle to Firebase Crashlytics.
///
/// The crashes of the native code and of the Android side are collected by Crashlytics itself;
/// this covers the errors of Dart, including the panics of Rust that reach Dart as exceptions.
abstract final class CrashReporting {
  static FirebaseCrashlytics? _crashlytics;

  /// Starts Firebase on Android. A build without a Firebase configuration reports nothing.
  static Future<void> initialize() async {
    if (!Platform.isAndroid) return;
    try {
      await Firebase.initializeApp();
    } on Exception catch (error) {
      debugPrint('Firebase is not available: $error');
      return;
    }
    _crashlytics = FirebaseCrashlytics.instance;
  }

  /// Records an error that Flutter reported.
  static void recordFlutterError(FlutterErrorDetails details) {
    _crashlytics?.recordFlutterError(details);
  }
}
