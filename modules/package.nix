{ ... }:
{
  perSystem =
    { config, pkgs, ... }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
    in
    {
      packages.monana = pkgs.rustPlatform.buildRustPackage {
        pname = "monana";
        version = cargoToml.package.version;

        src = ../.;

        outputs = [
          "out"
          "man"
        ];

        cargoLock = {
          lockFile = ../Cargo.lock;
        };

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

        buildInputs =
          with pkgs;
          [
            dbus
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Foundation
            pkgs.darwin.apple_sdk.frameworks.UserNotifications
          ];

        postInstall = ''
          # Install man page to man output
          install -Dm644 monana.1 $man/share/man/man1/monana.1
        '';

        meta = with pkgs.lib; {
          description = cargoToml.package.description;
          homepage = cargoToml.package.homepage;
          license = licenses.mit;
          maintainers = with maintainers; [ nilp0inter ];
          mainProgram = "monana";
          outputsToInstall = [
            "out"
            "man"
          ];
        };
      };

      # Make monana the default package
      packages.default = config.packages.monana;
    };
}
