abstract final class GameExtensions {
  static const Set<String> extensions = {
    '3dsx',
    'elf',
    'axf',
    'cci',
    'cxi',
    'app',
  };
  static const Set<String> badExtensions = {
    'rar',
    'zip',
    '7z',
    'torrent',
    'tar',
    'gz',
  };
  static const Set<String> allExtensions = {...extensions, ...badExtensions};
}
