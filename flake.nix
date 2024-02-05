{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    import-cargo.url = "github:edolstra/import-cargo";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    import-cargo,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {inherit system overlays;};
      inherit (import-cargo.builders) importCargo;

      rust = pkgs.rust-bin.nightly.latest.default;
      postgresql = pkgs.postgresql_16.withPackages (pkgs: [pkgs.pgvector]);

      devInputs =
        (with pkgs; [
          alejandra
          black
          poetry
          sea-orm-cli
        ])
        ++ [
          postgresql
          pgstart
          pgstop
        ];
      buildInputs = [pkgs.openssl];
      nativeBuildInputs = [rust pkgs.pkg-config];

      picsavbot = pkgs.stdenv.mkDerivation {
          name = "picsavbot";
          src = self;

          inherit buildInputs;

          nativeBuildInputs =
            nativeBuildInputs
            ++ [
              (importCargo {
                lockFile = ./Cargo.lock;
                inherit pkgs;
              })
              .cargoHome
            ];

          buildPhase = ''
            cargo build --release --offline
          '';

          installPhase = ''
            install -Dm775 ./target/release/picsavbot $out/bin/picsavbot
          '';
        };

      pgstart = pkgs.writeShellScriptBin "pgstart" ''
        if [ ! -d $PGHOST ]; then
          mkdir -p $PGHOST
        fi
        if [ ! -d $PGDATA ]; then
          echo 'Initializing postgresql database...'
          LC_ALL=C.utf8 initdb $PGDATA --auth=trust >/dev/null
        fi
        OLD_PGDATABASE=$PGDATABASE
        export PGDATABASE=postgres
        pg_ctl start -l $LOG_PATH -o "-c listen_addresses= -c unix_socket_directories=$PGHOST"
        psql -tAc "SELECT 1 FROM pg_database WHERE datname = 'picsavbot'" | grep -q 1 || psql -tAc 'CREATE DATABASE "picsavbot"'
        export PGDATABASE=$OLD_PGDATABASE
      '';

      pgstop = pkgs.writeShellScriptBin "pgstop" ''
        pg_ctl -D $PGDATA stop | true
      '';
    in {
      packages = {
        default = picsavbot;
      };

      devShells.default = pkgs.mkShell {
        LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath [
          pkgs.stdenv.cc.cc
        ]}";
        buildInputs = devInputs ++ buildInputs ++ nativeBuildInputs;

        shellHook = ''
          export PGDATA=$PWD/postgres/data
          export PGHOST=$PWD/postgres
          export LOG_PATH=$PWD/postgres/LOG
          export PGDATABASE=picsavbot
          export DATABASE_URL=postgresql:///picsavbot?host=$PWD/postgres;
        '';
      };
    });
}
