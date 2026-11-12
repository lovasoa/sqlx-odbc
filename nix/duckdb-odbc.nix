{
  autoPatchelfHook,
  fetchzip,
  lib,
  stdenv,
  unixodbc,
}:

let
  version = "1.5.5.0";

  releaseAssets = {
    x86_64-linux = {
      name = "duckdb_odbc-linux-amd64.zip";
      hash = "sha256-Tm9IATK65xV2dsZDSOdteDucKDxb2bWj7cTGrV3wIUg=";
    };
    aarch64-linux = {
      name = "duckdb_odbc-linux-arm64.zip";
      hash = "sha256-orn3LV3kBshj17JLxpQYSgLkS1YlnIHEjYmjyXd9ieA=";
    };
  };

  asset =
    releaseAssets.${stdenv.hostPlatform.system}
      or (throw "DuckDB ODBC ${version} has no prebuilt driver for ${stdenv.hostPlatform.system}");
in
stdenv.mkDerivation {
  pname = "duckdb-odbc-prebuilt";
  inherit version;

  # SQLPage installs this prebuilt release, so the reproductions use the same binary
  # instead of the source build in nixpkgs.
  # https://github.com/sqlpage/SQLPage/blob/41dd9db2f66b9cd9909b60a3443cfd4d939e24db/scripts/install-duckdb-odbc.sh
  src = fetchzip {
    url = "https://github.com/duckdb/duckdb-odbc/releases/download/v${version}/${asset.name}";
    inherit (asset) hash;
    stripRoot = false;
  };

  nativeBuildInputs = [ autoPatchelfHook ];

  buildInputs = [
    stdenv.cc.cc.lib
    unixodbc
  ];

  installPhase = ''
    runHook preInstall
    install -Dm755 libduckdb_odbc.so -t $out/lib
    runHook postInstall
  '';

  passthru.driver = "lib/libduckdb_odbc.so";

  meta = {
    description = "Prebuilt DuckDB ODBC driver, pinned to the release SQLPage ships";
    homepage = "https://github.com/duckdb/duckdb-odbc";
    license = lib.licenses.mit;
    platforms = builtins.attrNames releaseAssets;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
}
