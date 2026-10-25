{
  description = "Development environment for Azahar";

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.zst";
  };

  outputs = inputs: {
    devShells = builtins.mapAttrs (system: _:
      let
        pkgs = import inputs.nixpkgs {
          inherit system;
          config = {
            allowUnfree = true;
            android_sdk.accept_license = true;
          };
        };

        android = pkgs.androidenv.composeAndroidPackages {
          platformVersions = [ "30" "31" "34" "35" "36" "37" ];
          buildToolsVersions = [ "36.0.0" "37.0.0" ];
          ndkVersions = [ "28.2.13676358" ];
          cmakeVersions = [ "3.22.1" ];
          includeNDK = true;
        };

        androidHome = "${android.androidsdk}/libexec/android-sdk";

        gccWithClang = pkgs.runCommand "gcc-with-clang" { } ''
          mkdir -p $out/bin
          for tool in gcc:${pkgs.gcc}/bin/gcc g++:${pkgs.gcc}/bin/g++ clang:${pkgs.clang}/bin/clang gcc-ar:${pkgs.gcc.cc}/bin/gcc-ar gcc-ranlib:${pkgs.gcc.cc}/bin/gcc-ranlib gcc-nm:${pkgs.gcc.cc}/bin/gcc-nm; do
            printf '#!${pkgs.runtimeShell}\nexec %s "$@"\n' "''${tool#*:}" > $out/bin/''${tool%%:*}
            chmod +x $out/bin/''${tool%%:*}
          done
        '';
      in
      {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustup
            cmake
            ninja
            ccache
            pkg-config
            fvm
            jdk17
          ];

          buildInputs = with pkgs; [
            gtk3
            libevdev
            alsa-lib
            libepoxy
            fontconfig
            sysprof
            systemd
          ];

          shellHook = ''
            repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
            if [ -d "$repo_root/.fvm/flutter_sdk/bin" ]; then
              export PATH="$repo_root/.fvm/flutter_sdk/bin:$PATH"
            fi
            export NIX_LDFLAGS="$NIX_LDFLAGS -rpath-link ${pkgs.libepoxy}/lib -rpath-link ${pkgs.fontconfig.lib}/lib"
          '';

          GCC_BIN_DIR = "${gccWithClang}/bin";
          CMAKE_GENERATOR = "Ninja";
          JAVA_HOME = "${pkgs.jdk17}";
          ANDROID_HOME = androidHome;
          ANDROID_SDK_ROOT = androidHome;
          GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidHome}/build-tools/37.0.0/aapt2";
        };
      }) inputs.nixpkgs.legacyPackages;
  };
}
