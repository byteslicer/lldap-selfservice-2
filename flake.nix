{
  description = "LLDAP community self-service portal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        lldap-selfservice = pkgs.rustPlatform.buildRustPackage {
          pname = "lldap-selfservice";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [ openssl ];

          postInstall = ''
            mkdir -p $out/share/lldap-selfservice
            cp -r static $out/share/lldap-selfservice/
            cp config.example.toml $out/share/lldap-selfservice/
          '';

          meta = with pkgs.lib; {
            description = "Community self-service portal for LLDAP";
            license = licenses.mit;
          };
        };
      in {
        packages.default = lldap-selfservice;
        packages.lldap-selfservice = lldap-selfservice;

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            pkg-config
            openssl
            sqlx-cli
          ];
          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"
          '';
        };

        nixosModules.default = import ./nix/module.nix;
        nixosModules.lldap-selfservice = import ./nix/module.nix;
      });
}
