{ pkgs, lib, config, ... }:

let
  cfg = config.integrations.mcp-cli;

  processServer = name: server:
    if server.url != null then
      {
        url = server.url;
      } // lib.optionalAttrs (server.headers != {}) {
        headers = server.headers;
      }
    else
      if server.command != null then
        {
          command = server.command;
          args = server.args;
        } // lib.optionalAttrs (server.env != {}) {
          env = server.env;
        } // lib.optionalAttrs (server.cwd != null) {
          cwd = server.cwd;
        }
      else
        throw "MCP server '${name}' must have either 'url' or 'command' defined.";

  processedServers = lib.mapAttrs processServer cfg.servers;
in
{
  options.integrations.mcp-cli = {
    enable = lib.mkEnableOption "mcp-cli integration";

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = "The mcp-cli package to use. If null, it is assumed to be in the path.";
    };

    servers = lib.mkOption {
      type = lib.types.attrsOf (lib.types.submodule {
        options = {
          command = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            description = "The command to execute (for stdio servers).";
          };
          args = lib.mkOption {
            type = lib.types.listOf lib.types.str;
            default = [];
            description = "Arguments to pass to the command.";
          };
          env = lib.mkOption {
            type = lib.types.attrsOf lib.types.str;
            default = {};
            description = "Environment variables.";
          };
          cwd = lib.mkOption {
             type = lib.types.nullOr lib.types.str;
             default = null;
             description = "Current working directory.";
          };
          url = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            description = "URL for HTTP/SSE servers.";
          };
          headers = lib.mkOption {
            type = lib.types.attrsOf lib.types.str;
            default = {};
            description = "Headers for HTTP/SSE servers.";
          };
        };
      });
      default = {};
      description = "MCP servers configuration.";
    };
  };

  config = lib.mkIf cfg.enable {
    packages = lib.optional (cfg.package != null) cfg.package;

    env.MCP_CONFIG_PATH = "${config.devenv.root}/mcp_servers.json";

    files."mcp_servers.json".json = {
      mcpServers = processedServers;
    };
  };
}
