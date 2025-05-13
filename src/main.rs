use clap::Parser;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

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

fn get_file_path<P: AsRef<Path>>(
    dir_path: P,
    string_to_contain: &str,
) -> Result<Option<PathBuf>, io::Error> {
    let mut files_with_metadata: Vec<(PathBuf, std::fs::Metadata)> = Vec::new();
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let metadata = fs::metadata(&path)?;
            files_with_metadata.push((path, metadata));
        }
    }

    files_with_metadata.sort_by(|(_, a), (_, b)| {
        b.created()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(&a.created().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
    });

    for (file_path, _) in files_with_metadata {
        if let Some(file_name) = file_path.file_name() {
            if let Some(file_name_str) = file_name.to_str() {
                if file_name_str.contains(string_to_contain) {
                    return Ok(Some(file_path));
                }
            }
        }
    }

    Ok(None)
}

fn main() {
    let args = Args::parse();

    /* let agent deal with it
    let path: &Path = args.dir.as_ref();
    if !(path.is_dir() || (!path.exists() && path.parent().map_or(false, |p| p.is_dir()))) {
        eprintln!("{} is not a valid directory", args.dir);
        return;
    }
    */

    let directory_path = "C:\\ProgramData\\Battle.net\\Agent\\Agent.9124\\Logs";
    let string_to_look_for = "Agent-";

    let port = fs::read_to_string("C:\\ProgramData\\Battle.net\\Agent\\Agent.dat").unwrap();
    let auth: String;
    //let pid: String;

    match get_file_path(directory_path, string_to_look_for) {
        Ok(file_path) => match file_path {
            Some(file_path) => {
                let auth_re = Regex::new(r"authorization.: .(\d*)").unwrap();
                //let pid_re = Regex::new(r"pid.: (\d*)").unwrap();
                let log_file = fs::read_to_string(&file_path).unwrap();

                let auth_cap = auth_re.captures_iter(&log_file); //.unwrap();
                //let pid_re = pid_re.captures(&log_file).unwrap();

                // idk rust very well, but surely this is normal right
                auth = auth_cap
                    .last()
                    .unwrap()
                    .get(1)
                    .unwrap()
                    .as_str()
                    .to_string();
                //pid = (&pid_re[1]).to_string();

                println!("Found log file: {}", file_path.display());
                println!("Authorization: {}", auth);
                //println!("PID: {}", pid);
            }
            None => {
                eprintln!("Could not find log file.");
                return;
            }
        },
        Err(err) => {
            eprintln!("Error finding files: {}", err);
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

    //println!("Done... install started your battle.net client will reflect the install shortly.");
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
