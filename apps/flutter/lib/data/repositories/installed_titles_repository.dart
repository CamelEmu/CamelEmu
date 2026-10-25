import 'package:azahar_for_flutter/azahar_for_flutter.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

class InstalledTitlesRepository {
  static const _iconsName = 'icons';

  Future<List<Game>> scan(String? gamesDirectory) async {
    final cache = await getApplicationCacheDirectory();
    return scanGames(
      request: ScanRequest(
        gamesDirectory: gamesDirectory,
        iconsDirectory: p.join(cache.path, _iconsName),
      ),
    );
  }
}
