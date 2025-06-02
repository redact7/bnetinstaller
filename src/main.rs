use clap::Parser;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use windows::{Win32::Foundation::*, Win32::System::Diagnostics::ToolHelp::*, core::*};

#[derive(Parser, Debug)]
#[command(long_about = None)]
struct Args {
    /// TACT Product
    #[arg(short, long)]
    prod: String,

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

fn get_modified_latest<P: AsRef<Path>>(dir_path: P) -> Result<Vec<PathBuf>> {
    let mut files_with_modified_time: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if let Ok(modified) = metadata.modified() {
            files_with_modified_time.push((path, modified));
        }
    }

    files_with_modified_time.sort_by(|(_, a), (_, b)| b.cmp(a));

    let sorted_paths: Vec<PathBuf> = files_with_modified_time
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

fn find_file<'a, I>(paths: I, string_to_contain: &str) -> Option<PathBuf>
where
    I: IntoIterator<Item = &'a PathBuf>,
{
    for path in paths {
        if path.is_file() {
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

fn get_pid_from_exe(exe_name: &str) -> Option<u32> {
    let mut pid: Option<u32> = None;
    let mut pe32: PROCESSENTRY32 = unsafe { std::mem::zeroed() };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.unwrap();
    if snapshot.is_invalid() {
        println!("Error creating snapshot: {:?}", unsafe { GetLastError() });
        return None;
    }

    pe32.dwSize = size_of::<PROCESSENTRY32>() as u32;
    if unsafe { Process32First(snapshot, &mut pe32).is_err() } {
        println!("Error retrieving first process information: {:?}", unsafe {
            GetLastError()
        });
    }

    loop {
        let current_exe =
            unsafe { std::ffi::CStr::from_ptr(pe32.szExeFile.as_ptr()).to_string_lossy() };
        //println!("{}", current_exe);

        if current_exe.eq_ignore_ascii_case(exe_name) {
            pid = Some(pe32.th32ProcessID);
            //println!("{:?}", pid);
            break;
        }

        if unsafe { Process32Next(snapshot, &mut pe32) }.is_err() {
            break;
        }
    }

    unsafe {
        let _ = CloseHandle(snapshot);
    };
    pid
}

fn force_authorization(pid: &str, port: &str) -> Option<String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("pheonix-agent/1.0"));
    headers.insert("pid", HeaderValue::from_str(&pid).unwrap());

    let client = reqwest::blocking::Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{}/agent", port))
        .headers(headers.clone())
        .send()
        .expect("Failed to send /agent request... is your battle.net open?");

    if res.status().is_success() {
        println!("Successful /agent request");
    } else {
        eprintln!("Unsuccessful /agent request: {:?}", res);
        return None;
    }

    let agent_data: Agent = serde_json::from_slice::<Agent>(res.text().unwrap().as_ref()).unwrap();
    Some(agent_data.authorization)
}

fn main() {
    let args = Args::parse();
    let mut force_auth = false;

    /* let agent deal with it
    let path: &Path = args.dir.as_ref();
    if !(path.is_dir() || (!path.exists() && path.parent().map_or(false, |p| p.is_dir()))) {
        eprintln!("{} is not a valid directory", args.dir);
        return;
    }
    */

    let mut directory_path: PathBuf = PathBuf::from("C:\\ProgramData\\Battle.net\\Agent"); //\\Agent.9124\\Logs";
    match get_created_latest(&directory_path) {
        Ok(paths) => match find_directory(&paths, "Agent") {
            Some(agent_folder_path) => {
                directory_path = agent_folder_path;
                directory_path.push("Logs");
                //println!("Successfully constructed log path: {:?}", directory_path);
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
    let auth: String;
    let pid: String = get_pid_from_exe("battle.net.exe").unwrap().to_string();

    match get_modified_latest(directory_path) {
        Ok(paths) => match find_file(&paths, "Agent-") {
            Some(file_path) => {
                println!("Found log file: {}", file_path.display());

                let auth_re = Regex::new(r"authorization.: .(\d*)").unwrap();
                //let pid_re = Regex::new(r"pid.: (\d*)").unwrap();
                let log_file = fs::read_to_string(&file_path).unwrap();

                let auth_cap = auth_re.captures_iter(&log_file); //.unwrap();
                //let pid_re = pid_re.captures(&log_file).unwrap();

                // idk rust very well, but surely this is normal right
                // auth = auth_cap
                //     .last()
                //     .unwrap_or_else(|| panic!("Could not find authorization, {:?}", &log_file))
                //     .get(1)
                //     .unwrap()
                //     .as_str()
                //     .to_string();
                //
                match auth_cap.last() {
                    Some(v) => {
                        auth = v.get(1).unwrap().as_str().to_string();
                    }
                    None => {
                        println!("Could not find auth in log file forcing authorization.");
                        println!("PID: {}", pid);
                        force_auth = true;
                        auth = force_authorization(&pid, &port).expect("Invalid /agent authorization.");
                    }
                }

                //pid = (&pid_re[1]).to_string();
                println!("Authorization: {}", auth);
                //println!("PID: {}", pid);
            }
            None => {
                eprintln!("Could not find log file.");
                return;
            }
        },
        Err(err) => {
            eprintln!("Error finding log files: {}", err);
            return;
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("pheonix-agent/1.0"));
    headers.insert("Authorization", HeaderValue::from_str(&auth).unwrap());
    //println!("{:?}", headers);

    let install_data = InstallData {
        instructions_patch_url: format!("http://us.patch.battle.net:1119/{}", args.prod.clone()),
        instructions_product: "NGDP".to_string(),
        priority: Priority {
            insert_at_head: true,
            value: 699,
        },
        uid: args.prod.clone(),
    };

    let install_final = InstallFinal {
        finalized: true,
        game_dir: args.dir,
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
        println!("Successful setup install request.");
    } else {
        eprintln!("Unsuccessful setup install request.");
        return;
    }

    if force_auth {
        headers.insert("Authorization", HeaderValue::from_str(&force_authorization(&pid, &port).unwrap().to_string()).unwrap());
        println!("Authorization: {}", auth);
    }
    let res = client
        .post(format!(
            "http://127.0.0.1:{}/install/{}",
            port,
            args.prod.clone()
        ))
        .headers(headers.clone())
        .body(serde_json::to_string(&install_final).unwrap())
        .send()
        .expect("Failed to send start install request... is your battle.net open?")
        .status()
        .is_success();
    if res {
        println!("Successful finalize install request.")
    } else {
        eprintln!("Unsuccessful finalize install request.");
        return;
    }

    loop {
        if force_auth
        {
            println!("Done! The game is ready to download, restart battle.net to see it's progress.");
            break;
        }
           
        // if force_auth {
        //     headers.insert("Authorization", HeaderValue::from_str(&force_authorization(&pid, &port).unwrap().to_string()).unwrap());
        //     println!("Authorization: {}", auth);
        // }
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
