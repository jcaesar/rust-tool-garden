{
  outputs = {nixpkgs, ...}: let
    inherit (nixpkgs.lib) genAttrs mapAttrs flip;
  in {
    packages = flip mapAttrs nixpkgs.legacyPackages (_: pkgs:
      genAttrs ["pnnsul" "relarc"] (
        name:
          pkgs.callPackage ({rustPlatform}:
            rustPlatform.buildRustPackage {
              inherit name;
              meta.mainProgram = name;
              src = ./${name}/.;
              cargoLock.lockFile = ./${name}/Cargo.lock;
            }) {}
      ));
  };
}
