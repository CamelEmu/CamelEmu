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
            printf '#!%s\nexec %s "$@"\n' ${pkgs.runtimeShell} ${pkgs.gcc}/bin/$tool > $out/bin/$tool
          done
          printf '#!%s\nexec %s "$@"\n' ${pkgs.runtimeShell} ${pkgs.clang}/bin/clang > $out/bin/clang
          chmod +x $out/bin/*
        '';
      in
      {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustup
            cmake
            ninja
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
            pcre2
            util-linux.dev
          ];

          CMAKE_GENERATOR = "Ninja";
          JAVA_HOME = "${pkgs.jdk17}";
          ANDROID_HOME = androidHome;
          ANDROID_SDK_ROOT = androidHome;
          GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidHome}/build-tools/37.0.0/aapt2";

          shellHook = ''
            if [ -f .fvmrc ]; then
              [ -x .fvm/flutter_sdk/bin/flutter ] || fvm install
              export PATH="$PWD/.fvm/flutter_sdk/bin:$PATH"
            fi
          '';
        };
      }) inputs.nixpkgs.legacyPackages;
  };
}
