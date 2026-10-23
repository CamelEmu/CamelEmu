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
          for tool in gcc g++; do
            printf '#!%s\nexec %s "$@"\n' ${pkgs.runtimeShell} ${pkgs.gcc13}/bin/$tool > $out/bin/$tool
          done
          printf '#!%s\nexec %s "$@"\n' ${pkgs.runtimeShell} ${pkgs.clang}/bin/clang > $out/bin/clang
          for tool in ar ranlib; do
            printf '#!%s\nplugin=$(%s/bin/gcc -print-prog-name=lto-wrapper)\nexec %s/bin/%s --plugin "$(dirname "$plugin")/liblto_plugin.so" "$@"\n' ${pkgs.runtimeShell} ${pkgs.gcc13} ${pkgs.binutils} $tool > $out/bin/$tool
          done
          chmod +x $out/bin/*
        '';
      in
      {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustup
            cmake
            ninja
            sccache
            pkg-config
            fvm
            jdk17
            gccWithClang
          ];

          buildInputs = with pkgs; [
            gtk3
            libevdev
            alsa-lib
            libsysprof-capture
            systemd
            systemd.dev
            libepoxy
            fontconfig
            pcre2
            util-linux.dev
            libselinux
          ];

          CMAKE_GENERATOR = "Ninja";
          CMAKE_C_COMPILER_LAUNCHER = "sccache";
          CMAKE_CXX_COMPILER_LAUNCHER = "sccache";
          JAVA_HOME = "${pkgs.jdk17}";
          ANDROID_HOME = androidHome;
          ANDROID_SDK_ROOT = androidHome;
          GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidHome}/build-tools/37.0.0/aapt2";

          shellHook = ''
            export AR=${gccWithClang}/bin/ar
            export RANLIB=${gccWithClang}/bin/ranlib
            export CMAKE_AR="$AR"
            export CMAKE_RANLIB="$RANLIB"

            for flag in $NIX_LDFLAGS; do
              case $flag in
                -L*) NIX_LDFLAGS="$NIX_LDFLAGS -rpath-link ''${flag#-L}" ;;
              esac
            done
            export NIX_LDFLAGS

            if [ -f .fvmrc ]; then
              [ -x .fvm/flutter_sdk/bin/flutter ] || fvm install
              export PATH="$PWD/.fvm/flutter_sdk/bin:$PATH"
            fi
          '';
        };
      }) inputs.nixpkgs.legacyPackages;
  };
}
