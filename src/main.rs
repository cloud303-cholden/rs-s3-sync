use aws_sdk_s3::{
    output::GetObjectOutput,
    Client as S3Client,
    types::{
        ByteStream,
        AggregatedBytes,
        DateTime,
    }
};
use dirs::{
    config_dir,
    document_dir,
};
use serde::Deserialize;
use std::{
    cmp::Ordering,
    collections::HashMap,
    convert::TryFrom,
    error::Error,
    ffi::OsStr,
    fs::{
        File,
        OpenOptions,
        read_to_string,
        read_dir,
    },
    io::Write,
    path::PathBuf,
    process::exit,
    time::SystemTime,
};

#[derive(Deserialize)]
struct Config {
    dir: Option<PathBuf>,
    aws_profile: Option<String>,
    aws_bucket: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>>{
    let config_path: PathBuf = match config_dir() {
        Some(p) => p,
        _ => {
            eprintln!("Failed to find configuration directory!");
            exit(1);
        }
    };
    let sync_config_path = config_path.join("obsidian").join("obsidian-sync.toml");
    let sync_config_string = match read_to_string(sync_config_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Failed to find sync configuration file!");
            exit(1);
        }
    };
    let sync_config: Config = match toml::from_str(&sync_config_string) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Failed to parse configuration file!");
            exit(1);
        }
    };
    let dir = match sync_config.dir {
        Some(d) => d,
        _ => get_default_dir().unwrap(),
    };
    let local_files: Vec<PathBuf> = read_dir(&dir)
        .map(|res| res.map(|e| e.unwrap().path()))
        .unwrap()
        .collect::<Vec<PathBuf>>();

    let profile = match &sync_config.aws_profile {
        Some(p) => p,
        _ => "default",
    };

    let config = aws_config::from_env()
        .profile_name(profile)
        .load()
        .await;
    let client = S3Client::new(&config);

    let remote_files = client
        .list_objects_v2()
        .bucket(&sync_config.aws_bucket)
        .send()
        .await?;

    let mut remote_files: HashMap<String, Option<&DateTime>> = remote_files
        .contents()
        .unwrap()
        .iter()
        .map(|o| (
            o.key().unwrap().to_string(),
            o.last_modified(),
        ))
        .collect();

    for file in local_files.iter() {
        if file.extension() != Some(OsStr::new("md")) {
            continue
        }
        let key = file.file_name().unwrap().to_str().unwrap();

        match remote_files.remove(key) {
            Some(d) => {
                match SystemTime::try_from(d.unwrap().to_owned())?
                    .cmp(&file.metadata()?.modified()?) {
                    Ordering::Less => {
                        println!("Uploading newer: {}", &key);
                        put_object(
                            &client, &sync_config.aws_bucket, file, key
                        ).await?
                    },
                    Ordering::Greater => {
                        println!("Downloading newer: {}", &key);
                        let response = get_object(&client, &sync_config.aws_bucket, key)
                            .await?;
                        let agg_bytes = response
                            .body
                            .collect()
                            .await
                            .map(AggregatedBytes::into_bytes)
                            .unwrap();
                            
                        let mut f = OpenOptions::new().write(true).truncate(true).open(file)?;
                        f.write_all(&agg_bytes)?;
                        f.flush()?;
                    },
                    Ordering::Equal => (),
                }
            }
            _ => {
                println!("Uploading missing: {}", &key);
                put_object(
                    &client, &sync_config.aws_bucket, file, key
                ).await?
            },
        }

    };

    for missing_file in remote_files.keys() {
        println!("Downloading missing: {}", &missing_file);
        let response = get_object(&client, &sync_config.aws_bucket, missing_file)
            .await?;
        let agg_bytes = response
            .body
            .collect()
            .await
            .map(AggregatedBytes::into_bytes)
            .unwrap();
            
        let download_path = dir.join(missing_file);
        let mut f = File::create(download_path)?;
        f.write_all(&agg_bytes)?;
        f.flush()?;
    }
    Ok(())
}

fn get_default_dir() -> Result<PathBuf, Box<dyn Error>> {
    match document_dir() {
        Some(d) => Ok(d.join("Obsidian Vault")),
        _ => Err("Failed to find default Obsidian directory!".into()),
    }
}

async fn put_object(
    client: &S3Client,
    bucket_name: &str,
    file: &PathBuf,
    key: &str,
) -> Result<(), Box<dyn Error>> {
    let body = ByteStream::from_path(&file).await;
    client
        .put_object()
        .bucket(bucket_name)
        .key(key)
        .body(body.unwrap())
        .send()
        .await?;

    Ok(())
}

async fn get_object(
    client: &S3Client,
    bucket_name: &str,
    key: &str,
) -> Result<GetObjectOutput, Box<dyn Error>> {
    let response = client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await?;

    Ok(response)
}
