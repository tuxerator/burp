let
  craneLib = crane.mkLib pkgs;
in
{
  app = craneLib.buildPackage {
    src = ./.;
    nativeBuildInputs = [ pkgs.pkg-config ];
    PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
  };
}
