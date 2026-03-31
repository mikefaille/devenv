{ pkgs, config, lib, ... }:

let
  cfg = config.languages.dart;
in
{
  options.languages.dart = {
    enable = lib.mkEnableOption "tools for Dart development";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.dart;
      defaultText = lib.literalExpression "pkgs.dart";
      description = "The Dart package to use.";
    };

    enableFlutterMcp = lib.mkEnableOption "Dart and Flutter MCP server";
  };

  config = lib.mkIf cfg.enable {
    packages = [
      cfg.package
    ];

    integrations.mcp-cli.enable = lib.mkIf cfg.enableFlutterMcp true;

    integrations.mcp-cli.servers.dart = lib.mkIf cfg.enableFlutterMcp {
      command = "dart";
      args = [ "mcp-server" "--force-roots-fallback" ];
    };
  };
}
