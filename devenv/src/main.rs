use devenv::{
    cli::{Cli, Commands, ContainerCommand, InputsCommand, ProcessesCommand, TasksCommand},
    config,
    devenv::ProcessOptions,
    log, nix_backend, Devenv,
};
use miette::{bail, IntoDiagnostic, Result, WrapErr};
use std::{os::unix::process::CommandExt, process::Command, sync::Arc};
use tempfile::TempDir;
use tokio::sync::OnceCell;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_and_resolve_options();

    if cli.command.is_none() && cli.profile.is_empty() {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        cmd.print_help().into_diagnostic()?;
        return Ok(());
    }

    if let Some(Commands::Direnvrc) = cli.command {
        print!("{}", *devenv::DIRENVRC);
        return Ok(());
    }

    let level = if cli.global_options.verbose {
        log::Level::Debug
    } else if cli.global_options.quiet {
        log::Level::Silent
    } else {
        log::Level::default()
    };

    log::init_tracing(level, cli.global_options.log_format);

    let mut config = config::Config::load()?;
    for input in cli.global_options.override_input.chunks_exact(2) {
        config
            .override_input_url(&input[0].clone(), &input[1].clone())
            .wrap_err_with(|| {
                format!(
                    "Failed to override input {} with URL {}",
                    &input[0], &input[1]
                )
            })?;
    }

    // Initialize Nix backend early for profile validation
    let devenv_root = std::env::current_dir()
        .into_diagnostic()
        .wrap_err("Failed to get current directory")?;
    let devenv_dotfile = devenv_root.join(".devenv");

    let xdg_dirs = xdg::BaseDirectories::with_prefix("devenv");

    let cachix_trusted_keys = xdg_dirs.get_data_home().expect("Failed to get XDG data home").join("cachix_trusted_keys.json");
    let paths = nix_backend::DevenvPaths {
        root: devenv_root.clone(),
        dotfile: devenv_dotfile.clone(),
        dot_gc: devenv_dotfile.join("gc"),
        home_gc: xdg_dirs.get_data_home().expect("Failed to get XDG data home").join("gc"),
        cachix_trusted_keys,
    };
    let secretspec_resolved = Arc::new(OnceCell::new());
    let nix: Arc<Box<dyn nix_backend::NixBackend>> =
        match config.backend {
            config::NixBackendType::Nix => Arc::new(Box::new(
                devenv::nix::Nix::new(
                    config.clone(),
                    cli.global_options.clone(),
                    paths,
                    secretspec_resolved.clone(),
                )
                .await?,
            )),
            #[cfg(feature = "snix")]
            config::NixBackendType::Snix => Arc::new(Box::new(
                devenv::snix_backend::SnixBackend::new(
                    config.clone(),
                    cli.global_options.clone(),
                    paths,
                )
                .await?,
            )),
        };

    // Validate profiles before any command processing
    if !cli.profile.is_empty() {
        let available_profiles = nix.get_available_profiles().await?;
        if let Err(e) = cli.validate_profiles(&available_profiles) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    let command = cli.command.clone();

    let mut options = devenv::DevenvOptions {
        config,
        global_options: Some(cli.global_options),
        profiles: cli.profile,
        devenv_root: Some(devenv_root),
        devenv_dotfile: Some(devenv_dotfile.clone()),
        ..Default::default()
    };

    let _tmpdir = if let Some(Commands::Test {
        dont_override_dotfile,
    }) = &command
    {
        let pwd = std::env::current_dir()
            .into_diagnostic()
            .wrap_err("Failed to get current directory")?;
        let tmpdir = TempDir::with_prefix_in(".devenv.", pwd)
            .into_diagnostic()
            .wrap_err("Failed to create temporary directory")?;
        if !dont_override_dotfile {
            let file_name = tmpdir
                .path()
                .file_name()
                .ok_or_else(|| miette::miette!("Temporary directory path is invalid"))?
                .to_str()
                .ok_or_else(|| {
                    miette::miette!("Temporary directory name contains invalid Unicode")
                })?;
            info!("Overriding .devenv to {}", file_name);
            options.devenv_dotfile = Some(tmpdir.path().to_path_buf());
        }
        Some(tmpdir)
    } else {
        None
    };

    let mut devenv = Devenv::new(options, nix, secretspec_resolved).await;

    match command {
        Some(cmd) => match cmd {
            Commands::Shell { cmd, ref args } => match cmd {
                Some(cmd) => devenv.exec_in_shell(Some(cmd), args).await,
                None => devenv.shell().await,
            },
            Commands::Test { .. } => devenv.test().await,
            Commands::Container {
                registry,
                copy,
                docker_run,
                copy_args,
                name,
                command,
            } => {
                let command = if let Some(name) = name {
                    if copy {
                        warn!(
                            devenv.is_user_message = true,
                            "The --copy flag is deprecated. Use `devenv container copy` instead."
                        );
                        ContainerCommand::Copy { name }
                    } else if docker_run {
                        warn!(
                            devenv.is_user_message = true,
                            "The --docker-run flag is deprecated. Use `devenv container run` instead."
                        );
                        ContainerCommand::Run { name }
                    } else {
                        warn!(
                            devenv.is_user_message = true,
                            "Calling `devenv container` without a subcommand is deprecated. Use `devenv container build {name}` instead."
                        );
                        ContainerCommand::Build { name }
                    }
                } else {
                    if let Some(cmd) = command {
                        cmd
                    } else {
                        bail!(
                            "No container subcommand provided. Use `devenv container build` or specify a command."
                        )
                    }
                };

                match command {
                    ContainerCommand::Build { name } => {
                        let path = devenv.container_build(&name).await?;
                        println!("{path}");
                    }
                    ContainerCommand::Copy { name } => {
                        devenv
                            .container_copy(&name, &copy_args, registry.as_deref())
                            .await?;
                    }
                    ContainerCommand::Run { name } => {
                        devenv
                            .container_run(&name, &copy_args, registry.as_deref())
                            .await?;
                    }
                }

                Ok(())
            }
            Commands::Init { target } => devenv.init(&target),
            Commands::Generate { .. } => match which::which("devenv-generate") {
                Ok(devenv_generate) => {
                    let error = Command::new(devenv_generate)
                        .args(std::env::args().skip(1).filter(|arg| arg != "generate"))
                        .exec();
                    miette::bail!("failed to execute devenv-generate {error}");
                }
                Err(_) => {
                    miette::bail!(indoc::formatdoc! {"
                    devenv-generate was not found in PATH

                    It was moved to a separate binary due to https://github.com/cachix/devenv/issues/1733

                    For now, use the web version at https://devenv.new
                "})
                }
            },
            Commands::Search { name } => devenv.search(&name).await,
            Commands::Gc {} => devenv.gc().await,
            Commands::Info {} => devenv.info().await,
            Commands::Repl {} => devenv.repl().await,
            Commands::Build { attributes } => devenv.build(&attributes).await,
            Commands::Update { name } => devenv.update(&name).await,
            Commands::Up { processes, detach }
            | Commands::Processes {
                command: ProcessesCommand::Up { processes, detach },
            } => {
                let options = ProcessOptions {
                    detach,
                    log_to_file: detach,
                    ..Default::default()
                };
                devenv.up(processes, &options).await
            }
            Commands::Processes {
                command: ProcessesCommand::Down {},
            } => devenv.down().await,
            Commands::Tasks { command } => match command {
                TasksCommand::Run { tasks, mode } => devenv.tasks_run(tasks, mode).await,
                TasksCommand::List {} => devenv.tasks_list().await,
            },
            Commands::Inputs { command } => match command {
                InputsCommand::Add { name, url, follows } => {
                    devenv.inputs_add(&name, &url, &follows).await
                }
            },
            Commands::Assemble => devenv.assemble(false).await,
            Commands::PrintDevEnv { json } => devenv.print_dev_env(json).await,
            Commands::GenerateJSONSchema => {
                config::write_json_schema()
                    .await
                    .wrap_err("Failed to generate JSON schema")?;
                Ok(())
            }
            Commands::Mcp {} => {
                let config = devenv.config.read().await.clone();
                devenv::mcp::run_mcp_server(config, devenv.nix.clone()).await
            }
            Commands::Direnvrc => unreachable!(),
        },
        None => {
            if devenv.profiles.contains(&"ci".to_string()) {
                return devenv.test().await;
            }
            devenv.shell().await
        }
    }
}