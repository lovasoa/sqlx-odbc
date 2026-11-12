{
  lib,
  writeTextDir,
  drivers,
}:

let
  driverSection = name: package: ''
    [${name}]
    Driver = ${package}/${package.passthru.driver}
  '';
in
writeTextDir "odbcinst.ini" (lib.concatStrings (lib.mapAttrsToList driverSection drivers))
