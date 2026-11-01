use clap::Parser;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::os::windows::process::CommandExt;
use windows::{core::*};
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Deserialize)]
struct InstallResponse {
    form: Form,
    // response_uri: String,
}

#[derive(Debug, Deserialize)]
struct Form {
    authentication: ErrorOnly,
    eula: ErrorOnly,
    game_dir: GameDir,
    language: ErrorOnly,
    min_spec: ErrorOnly,
    tome_path: ErrorOnly,
}

#[derive(Debug, Deserialize)]
struct ErrorOnly {
    error: i32,
}

#[derive(Debug, Deserialize)]
struct GameDir {
    // default: String,
    // dirname: String,
    error: i32,
    // game_dir: String,
    // required_space: i64,
    required_space_info: RequiredSpaceInfo,
    // warning: i32,
}

#[derive(Debug, Deserialize)]
struct RequiredSpaceInfo {
    // size: i64,
    state: i32,
}

#[derive(Parser, Debug)]
#[command(long_about = None)]
struct Args {
    /// TACT Product
    #[arg(short, long)]
    prod: String,

    /// Agent UID
    #[arg(short, long)]
    uid: Option<String>,

    /// Game/Asset language
    #[arg(short, long)]
    lang: String,

    /// Installation Directory
    #[arg(short, long)]
    dir: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Priority {
    insert_at_head: bool,
    value: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Agent {
    authorization: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct InstallData {
    instructions_patch_url: String,
    instructions_product: String,
    priority: Priority,
    uid: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct InstallFinal {
    finalized: bool,
    game_dir: String,
    language: Vec<String>,
    selected_asset_locale: String,
    selected_locale: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct InstallProgress {
    progress: f32,
}

fn get_created_latest<P: AsRef<Path>>(dir_path: P) -> Result<Vec<PathBuf>> {
    let mut files_with_created_time: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if let Ok(created) = metadata.created() {
            files_with_created_time.push((path, created));
        }
    }

    files_with_created_time.sort_by(|(_, a), (_, b)| b.cmp(a));

    let sorted_paths: Vec<PathBuf> = files_with_created_time
        .into_iter()
        .map(|(path, _)| path)
        .collect();

    Ok(sorted_paths)
}

fn find_directory<'a, I>(paths: I, string_to_contain: &str) -> Option<PathBuf>
where
    I: IntoIterator<Item = &'a PathBuf>,
{
    for path in paths {
        if path.is_dir() {
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str.contains(string_to_contain) {
                        return Some(path.clone());
                    }
                }
            }
        }
    }
    None
}

fn get_agent_pid(agent_path: PathBuf) -> Option<u32> {
    let sys = sysinfo::System::new_all().unwrap();
    let agent_open = sys.processes_by_exact_name("Agent.exe".as_ref()).next();

    let pid;
    match agent_open {
        Some(proc) => pid = proc.pid().as_u32(),
        Option::None => {
            let agent = std::process::Command::new(&agent_path).creation_flags(CREATE_NO_WINDOW).spawn().expect("Failed to start Battle.net Agent. Make sure Battle.net is running.");
            pid = agent.id();
        },
    }

    Some(pid)
}

fn get_authorization(pid: u32, port: &str) -> Option<String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("pheonix-agent/1.0"));
    headers.insert("pid", HeaderValue::from_str(&pid.to_string()).unwrap());

    let client = reqwest::blocking::Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{}/agent", port))
        .headers(headers.clone())
        .send()
        .expect("Failed to send /agent request... is your battle.net open?");

    if res.status().is_success() {
        println!("Succesfully authorized with Battle.net Agent.");
    } else {
        eprintln!("Unsuccessful /agent request: {:?}", res);
        return None;
    }

    let agent_data: Agent = serde_json::from_slice::<Agent>(res.text().unwrap().as_ref()).unwrap();
    Some(agent_data.authorization)
}

fn error_code_to_support_article(code: i32, locale: &str) -> String {
    format!("http://nydus.battle.net/App/{locale}/setup/error/BLZBNTAGT{code:08X}")
}

fn check_agent_error(error: i32) -> bool {
    if error != 0 {
        eprintln!("Battle.net Agent error: {}", error_code_to_support_article(error, "enUS"));
        false
    } else {
        true
    }
}

fn main() {
    let args = Args::parse();

    let mut agent_path: PathBuf = PathBuf::from("C:\\ProgramData\\Battle.net\\Agent");
    match get_created_latest(&agent_path) {
        Ok(paths) => match find_directory(&paths, "Agent") {
            Some(agent_folder_path) => {
                agent_path = agent_folder_path;
                agent_path.push("Agent.exe")
            }
            None => {
                eprintln!("Could not find agent folder.");
            }
        },
        Err(err) => {
            eprintln!("Error finding agent directory: {}", err);
        }
    }

    let port = fs::read_to_string("C:\\ProgramData\\Battle.net\\Agent\\Agent.dat").unwrap();
    let pid = get_agent_pid(agent_path).unwrap();
    let auth = get_authorization(pid, &port).unwrap();

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("pheonix-agent/1.0"));
    headers.insert("Authorization", HeaderValue::from_str(&auth).unwrap());

    let install_data = InstallData {
        instructions_patch_url: format!("http://us.patch.battle.net:1119/{}", args.uid.as_deref().unwrap_or(&args.prod)),
        instructions_product: "NGDP".to_string(),
        priority: Priority {
            insert_at_head: true,
            value: 699,
        },
        uid: args.prod.clone(),
    };

    let install_loop = InstallFinal {
        finalized: false,
        game_dir: args.dir.clone(),
        language: vec![args.lang.clone()],
        selected_asset_locale: args.lang.clone(),
        selected_locale: args.lang.clone(),
    };

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(format!("http://127.0.0.1:{}/install", port))
        .headers(headers.clone())
        .body(serde_json::to_string(&install_data).unwrap())
        .send()
        .expect("Failed to send setup install request... is your battle.net open?")
        .status()
        .is_success();
    if res {
        println!("Preparing to install.");
    } else {
        eprintln!("Unsuccessful setup install request.");
        return;
    }

    loop {
        let res = client
            .post(format!(
                "http://127.0.0.1:{}/install/{}",
                port,
                args.prod.clone()
            ))
            .headers(headers.clone())
            .body(serde_json::to_string(&install_loop).unwrap())
            .send()
            .expect("Failed to send start install request... is your battle.net open?");
        if res.status().is_success() {
            let response = serde_json::from_slice::<InstallResponse>(res.text().unwrap().as_ref()).unwrap().form;

            if !check_agent_error(response.authentication.error) {break};
            if !check_agent_error(response.eula.error) {break};
            if !check_agent_error(response.language.error) {break};
            if !check_agent_error(response.min_spec.error) {break};
            if !check_agent_error(response.tome_path.error) {break};
            
            let progress = response.game_dir.required_space_info.state;
            if progress == 1009 { // TODO: 1009 should mean that the storage space calculation is finished. 1005 should mean in progress
                if check_agent_error(response.game_dir.error) {
                    break;
                } else {
                    return;
                }
            }
        } else {
            eprintln!("Unsuccessful loop install request.");
            return;
        }

        thread::sleep(Duration::from_secs(1));
    }

    let install_final = InstallFinal {
        finalized: true,
        game_dir: args.dir,
        language: vec![args.lang.clone()],
        selected_asset_locale: args.lang.clone(),
        selected_locale: args.lang.clone(),
    };

    let res = client
        .post(format!("http://127.0.0.1:{}/install/{}", port, args.prod.clone()))
        .headers(headers.clone())
        .body(serde_json::to_string(&install_final).unwrap())
        .send()
        .expect("Failed to send setup install request... is your battle.net open?")
        .status()
        .is_success();
    if res {
        println!("Starting install.");
    } else {
        eprintln!("Unsuccessful final install request.");
        return;
    }

    loop {
        let res = client
            .get(format!(
                "http://127.0.0.1:{}/install/{}",
                port,
                args.prod.clone()
            ))
            .headers(headers.clone())
            .send()
            .expect("Could not get install progress.");
        if res.status().is_success() {
            let progress: InstallProgress =
                serde_json::from_slice::<InstallProgress>(res.text().unwrap().as_ref()).unwrap();
            println!("Install progress: {}%", progress.progress * 100.0);
        } else {
            eprintln!("Unsuccessful install progress request.");
            return;
        }

        thread::sleep(Duration::from_secs(2));
    }
}
