{ pkgs, ... }:

{
  integrations.mcp-cli.enable = true;
  integrations.mcp-cli.servers = {
    test-server = {
      command = "echo";
      args = [ "hello" ];
    };
  };

  # Check that the config file is generated
  enterShell = ''
    if [ ! -f "$MCP_CONFIG_PATH" ]; then
      echo "MCP_CONFIG_PATH not found"
      exit 1
    fi
    if ! grep -q "test-server" "$MCP_CONFIG_PATH"; then
      echo "test-server not found in config"
      exit 1
    fi
  '';
}
