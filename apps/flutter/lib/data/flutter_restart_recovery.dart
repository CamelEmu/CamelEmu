import 'package:azahar_for_flutter/azahar_for_flutter.dart';

import '../app_services.dart';

/// Detects that Flutter was restarted while a game was running, destroys that game and restarts
/// the app. Returns true when the app is being restarted, so nothing else should be started.
///
/// A restart of Flutter, such as a hot restart, begins the Dart side anew, but the native process
/// and the game in it live on. A session left over at startup is exactly that case.
Future<bool> recoverFromFlutterRestart() async {
  try {
    await stopGame(persist: true);
  } on AzaharError_NoSession {
    return false;
  } on AzaharError {
    // The leftover game was destroyed, even though its changes could not all be written.
  }
  await AppServices.nativeBridge.restartApp();
  return true;
}
