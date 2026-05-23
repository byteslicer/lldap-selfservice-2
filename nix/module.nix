{ config, lib, pkgs, ... }:

let
  cfg = config.services.lldap-selfservice;
in
{
  options.services.lldap-selfservice = {
    enable = lib.mkEnableOption "LLDAP community self-service portal";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The lldap-selfservice package (build with nix build from the flake).";
    };

    configFile = lib.mkOption {
      type = lib.types.path;
      default = "${cfg.package}/share/lldap-selfservice/config.example.toml";
      description = "Application configuration TOML file.";
    };

    listenAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:8080";
      description = "HTTP listen address.";
    };

    publicBaseUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://selfservice.bambam.fun";
      description = "Public base URL for invite links.";
    };

    databaseUrl = lib.mkOption {
      type = lib.types.str;
      default = "postgres:///lldap_selfservice?host=/run/postgresql";
      description = "PostgreSQL connection URL for app state.";
    };

    lldapHttpUrl = lib.mkOption {
      type = lib.types.str;
      default = "http://127.0.0.1:17170";
      description = "LLDAP HTTP API base URL.";
    };

    lldapSetPasswordBin = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Path to lldap_set_password; defaults to lldap package binary.";
    };

    lldapServiceUsername = lib.mkOption {
      type = lib.types.str;
      default = "selfservice";
      description = "LLDAP service account username for GraphQL.";
    };

    sessionSecretFile = lib.mkOption {
      type = lib.types.path;
      description = "File containing session signing secret (32+ chars).";
    };

    servicePasswordFile = lib.mkOption {
      type = lib.types.path;
      description = "File containing LLDAP service account password.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.lldap-selfservice = {
      description = "LLDAP self-service portal";
      after = [ "network.target" "postgresql.service" "lldap.service" ];
      wants = [ "postgresql.service" "lldap.service" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        CONFIG_PATH = cfg.configFile;
        DATABASE_URL = cfg.databaseUrl;
        LISTEN = cfg.listenAddress;
        PUBLIC_BASE_URL = cfg.publicBaseUrl;
        LLDAP_HTTP_URL = cfg.lldapHttpUrl;
        LLDAP_SERVICE_USERNAME = cfg.lldapServiceUsername;
        LLDAP_SERVICE_PASSWORD_FILE = cfg.servicePasswordFile;
        SESSION_SECRET_FILE = cfg.sessionSecretFile;
        LLDAP_SET_PASSWORD_BIN = toString (
          cfg.lldapSetPasswordBin
          or "${pkgs.lldap}/bin/lldap_set_password"
        );
        STATIC_DIR = "${cfg.package}/share/lldap-selfservice/static";
        RUST_LOG = "info";
      };

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        DynamicUser = false;
        User = "lldap-selfservice";
        Group = "lldap-selfservice";
        StateDirectory = "lldap-selfservice";
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ "/var/lib/lldap-selfservice" ];
        AmbientCapabilities = "";
      };
    };

    users.users.lldap-selfservice = {
      isSystemUser = true;
      group = "lldap-selfservice";
      description = "LLDAP self-service portal";
    };
    users.groups.lldap-selfservice = { };
  };
}
