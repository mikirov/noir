use clap::Args;

use nargo::package::Package;
use nargo_toml::{get_package_manifest, resolve_workspace_from_toml, PackageSelection};
use noirc_abi::input_parser::Format;
use noirc_driver::{CompileOptions, CompiledProgram, NOIR_ARTIFACT_VERSION_STRING};
use noirc_frontend::graph::CrateName;

use super::NargoConfig;
use super::{
    compile_cmd::compile_bin_package,
    fs::{inputs::read_inputs_from_file, load_hex_data}
};
use crate::{backends::Backend, errors::CliError};

use nargo::constants::{PROOF_EXT, VERIFIER_INPUT_FILE};
use nargo::workspace::Workspace;


/// Generates intermediary artifacts for a given circuit
#[derive(Debug, Clone, Args)]
pub(crate) struct GenArtifactsCommand {
    /// The name of the package to generate artifacts for
    #[clap(long, conflicts_with = "workspace")]
    package: Option<CrateName>,

    /// Execute all packages in the workspace
    #[clap(long, conflicts_with = "package")]
    workspace: bool,

    #[clap(flatten)]
    compile_options: CompileOptions,

    /// The name of the toml file which contains the inputs for the verifier
    #[clap(long, short, default_value = VERIFIER_INPUT_FILE)]
    verifier_name: String,
}

pub(crate) fn run(
    backend: &Backend,
    args: GenArtifactsCommand,
    config: NargoConfig,
) -> Result<(), CliError> {
    let toml_path = get_package_manifest(&config.program_dir)?;
    let default_selection =
        if args.workspace { PackageSelection::All } else { PackageSelection::DefaultOrAll };
    let selection = args.package.map_or(default_selection, PackageSelection::Selected);
    let workspace = resolve_workspace_from_toml(
        &toml_path,
        selection,
        Some(NOIR_ARTIFACT_VERSION_STRING.to_string()),
    )?;

    let (np_language, opcode_support) = backend.get_backend_info()?;
    for package in &workspace {
        let program = compile_bin_package(
            &workspace,
            package,
            &args.compile_options,
            np_language,
            &opcode_support)?;

        generate_intermediary_artifacts(
            &backend,
            &workspace,
            &package,
            program,
            &args.verifier_name
        )?;
    }

    Ok(())
}

fn generate_intermediary_artifacts(    
    backend: &Backend,
    workspace: &Workspace,
    package: &Package,
    compiled_program: CompiledProgram,
    verifier_name: &str) -> Result<(), CliError> {

    let public_abi = compiled_program.abi.public_abi();
    let (public_inputs_map, return_value) =
        read_inputs_from_file(&package.root_dir, verifier_name, Format::Toml, &public_abi)?;

    let public_inputs = public_abi.encode(&public_inputs_map, return_value)?;

    let proof_path =
        workspace.proofs_directory_path().join(package.name.to_string()).with_extension(PROOF_EXT);

    let proof = load_hex_data(&proof_path)?;

    let (proof_as_fields, vk_hash, vk_as_fields) = backend.get_intermediate_proof_artifacts( &compiled_program.circuit, proof.as_slice(), public_inputs)?;

    println!("{:?}",proof_as_fields);
    println!("{:?}", vk_hash);
    println!("{:?}", vk_as_fields);

    // let contract_dir = workspace.contracts_directory_path(package);
    // create_named_dir(&contract_dir, "contract");
    // let contract_path = contract_dir.join("plonk_vk").with_extension("sol");

    // let path = write_to_file(contract_code.as_bytes(), &contract_path);
    // println!("[{}] Contract successfully created and located at {path}", package.name);

    Ok(())
}
