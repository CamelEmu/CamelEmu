import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app_services.dart';

/// Whether the core was built with the CPU and the memory of the Rust crates, which decides if the
/// switch of the Rust CPU is shown.
final rustCpuSupportedProvider = FutureProvider<bool>(
  (ref) => AppServices.nativeBridge.supportsRustCpu(),
);
