{
  outputs = {nixpkgs, ...}: {
    packages =
      builtins.mapAttrs (sys: pkgs: {
        default = pkgs.callPackage ({
          rustPlatform,
        }:
          rustPlatform.buildRustPackage {
            name = "pnnsul";
            meta.mainProgram = "pnnsul";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          }) {};
      })
      nixpkgs.legacyPackages;
  };
}
