use clap::{Parser, Subcommand};
use fdx_core::{demo, dictionary, engine, exporter, models::*, workbooks};
use std::{fs, path::{Path,PathBuf}, io::Write};

#[derive(Parser)]
#[command(name="fdx-cli",version,about="Preparation locale de classeurs Excel metier")]
struct Cli { #[command(subcommand)] command: Command }
#[derive(Subcommand)]
enum Command {
    Inspect { workbook: PathBuf, #[arg(long,default_value_t=1)] header_row: u32 },
    Analyze { #[arg(long)] config: PathBuf, #[arg(long)] decisions: Option<PathBuf>, #[arg(long)] output: PathBuf },
    Export { #[arg(long)] config: PathBuf, #[arg(long)] decisions: Option<PathBuf>, #[arg(long)] output: PathBuf, #[arg(long)] allow_issues: bool, #[arg(long)] overwrite: bool },
    Selftest { #[arg(long)] output: PathBuf, #[arg(long,default_value_t=1.0)] scale: f64 },
}
fn write_report(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?; temp.write_all(bytes)?; temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| error(e.to_string()))?; Ok(())
}
fn run(cli: Cli) -> Result<()> {
    let job = JobControl::default();
    match cli.command {
        Command::Inspect {workbook,header_row} => println!("{}",serde_json::to_string_pretty(&workbooks::inspect(&workbook,header_row,&job)?)?),
        Command::Selftest {output,scale} => {
            if !scale.is_finite() || scale <= 0.0 || scale > 100_000.0 { return Err(error("Echelle invalide.")); }
            let folder = tempfile::tempdir()?;
            let count = (3.0 * scale) as u32;
            let (source,dict) = demo::create(folder.path(),count)?;
            let data = workbooks::read_source(&source,&job)?;
            let info = workbooks::inspect(&source,1,&job)?;
            let mut config = ProjectConfig {source_path:source.to_string_lossy().into_owned(),original_path:source.to_string_lossy().into_owned(),sheet:info.suggested_sheet.clone(),..ProjectConfig::default()};
            config.fields = dictionary::parse_dictionary(&fs::read_to_string(dict)?).fields;
            config.mappings = dictionary::suggest_mappings(&info.sheets[0].headers,&config.fields);
            for mapping in &mut config.mappings { mapping.confirmed = true; }
            let result = engine::analyze(&config,&data,&Decisions::new(),&job)?;
            if result.row_count != count.max(3) { return Err(error("L'analyse n'a pas couvert toutes les lignes.")); }
            let destination = folder.path().join("resultat.xlsx");
            exporter::export_excel(&config,&result,&destination,true,false,&job)?;
            if workbooks::sha256(&source)? != data.source_hash { return Err(error("La source a ete modifiee.")); }
            write_report(&output,&serde_json::to_vec_pretty(&serde_json::json!({"ok":true,"rows":result.row_count,"findings":result.findings.len(),"changes":result.changes.len(),"source_unchanged":true}))?)?;
        }
        command => {
            let (config_path,decisions_path,output,export,allow,overwrite) = match command {
                Command::Analyze {config,decisions,output} => (config,decisions,output,false,false,false),
                Command::Export {config,decisions,output,allow_issues,overwrite} => (config,decisions,output,true,allow_issues,overwrite),
                _ => unreachable!(),
            };
            let config: ProjectConfig = serde_json::from_slice(&fs::read(&config_path)?)?;
            let decisions: Decisions = match decisions_path {Some(path)=>serde_json::from_slice(&fs::read(path)?)?,None=>Decisions::new()};
            let data = workbooks::read_source(Path::new(&config.source_path),&job)?;
            let result = engine::analyze(&config,&data,&decisions,&job)?;
            if export { exporter::export_excel(&config,&result,&output,allow,overwrite,&job)?; }
            else {
                let absolute = std::path::absolute(&output)?;
                for protected in [&config.source_path,&config.original_path,&config.dictionary_path] {
                    if !protected.is_empty() && (std::path::absolute(protected)? == absolute || fs::canonicalize(&output).ok().is_some_and(|p| fs::canonicalize(protected).ok() == Some(p))) { return Err(error("Le rapport ne peut pas remplacer un fichier source.")); }
                }
                if output.exists() { return Err(error("Ce rapport existe deja. Choisissez un nouveau nom.")); }
                write_report(&output,&serde_json::to_vec_pretty(&result)?)?;
            }
            println!("{}",serde_json::json!({"rows":result.row_count,"changes":result.changes.len(),"issues":result.findings.len()}));
        }
    }
    Ok(())
}
fn main() { if let Err(e) = run(Cli::parse()) { eprintln!("Erreur : {e}"); std::process::exit(1); } }
