use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::PathBuf,
    str::FromStr,
    time::SystemTime,
};

use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::{AggregatedBytes, ByteStream};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Config {
    paths: Vec<PathBuf>,
    aws: Aws,
}

#[derive(Deserialize, Serialize)]
struct Aws {
    profile: Option<String>,
    bucket: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = match std::env::var("S3_SYNC_CONFIG") {
        Ok(p) => PathBuf::from_str(&p).unwrap(),
        Err(_) => dirs::config_dir()
            .unwrap()
            .join("s3-sync")
            .join("config.toml"),
    };
    let config_str = std::fs::read_to_string(config_path).unwrap();
    let config: Config = toml::from_str(&config_str).unwrap();

    let mut aws_config_loader = aws_config::defaults(BehaviorVersion::v2023_11_09());
    if let Some(profile) = config.aws.profile {
        aws_config_loader = aws_config_loader.profile_name(profile);
    }
    let aws_config = aws_config_loader.load().await;
    let s3 = aws_sdk_s3::Client::new(&aws_config);
    let s3_bucket = config.aws.bucket;

    let resp = s3
        .list_objects_v2()
        .bucket(&s3_bucket)
        .send()
        .await?;

    let mut remote_files: HashSet<String> = resp
        .contents()
        .iter()
        .flat_map(|o| o.key())
        .map(String::from)
        .collect();
    let mut manifest = match remote_files.remove("manifest.json") {
        true => {
            let response = s3
                .get_object()
                .bucket(&s3_bucket)
                .key("manifest.json")
                .send()
                .await?;
            let agg_bytes = response
                .body
                .collect()
                .await
                .map(AggregatedBytes::into_bytes)
                .unwrap();
            let manifest: HashMap<String, SystemTime> = serde_json::from_slice(&agg_bytes).unwrap();
            manifest
        },
        false => HashMap::<String, SystemTime>::new(),
    };
    let mut local_files = HashSet::<String>::new();

    for dir in config.paths {
        let dir = walkdir::WalkDir::new(dir);
        for entry in dir
            .into_iter()
            .flatten()
            .filter(|e| !e.file_type().is_dir()) {
                let key = entry.path().to_str().unwrap();
                let mut last_modified = entry.metadata()?.modified()?;
                local_files.insert(String::from(key));
                match manifest.get_mut(key) {
                    Some(d) => {
                        match d.cmp(&&mut last_modified) {
                            std::cmp::Ordering::Less => {
                                println!("Uploading newer: {}", &key);
                                let body = ByteStream::from_path(entry.path()).await.unwrap();
                                s3
                                    .put_object()
                                    .bucket(&s3_bucket)
                                    .key(key)
                                    .body(body)
                                    .send()
                                    .await?;
                                *d = last_modified;
                            },
                            std::cmp::Ordering::Greater => {
                                println!("Downloading newer: {}", &key);
                                let response = s3
                                    .get_object()
                                    .bucket(&s3_bucket)
                                    .key(key)
                                    .send()
                                    .await?;
                                let agg_bytes = response
                                    .body
                                    .collect()
                                    .await
                                    .map(AggregatedBytes::into_bytes)
                                    .unwrap();

                                let mut f = std::fs::OpenOptions::new()
                                        .write(true)
                                        .truncate(true)
                                        .open(key)?;
                                f.write_all(&agg_bytes)?;
                                f.flush()?;
                            },
                            std::cmp::Ordering::Equal => (),
                        }
                    }
                    _ => {
                        println!("Uploading missing: {:?}", &key);
                        let body = ByteStream::from_path(entry.path()).await.unwrap();
                        s3
                            .put_object()
                            .bucket(&s3_bucket)
                            .key(key)
                            .body(body)
                            .send()
                            .await?;
                        manifest.insert(String::from(key), last_modified);
                    }
                }
        }
    }

    for missing_file in remote_files.difference(&local_files) {
        println!("Downloading missing: {}", &missing_file);
        let response = s3
            .get_object()
            .bucket(&s3_bucket)
            .key(missing_file)
            .send()
            .await?;
        let agg_bytes = response
            .body
            .collect()
            .await
            .map(AggregatedBytes::into_bytes)
            .unwrap();
            
        let mut f = std::fs::File::create(missing_file)?;
        f.write_all(&agg_bytes)?;
        f.flush()?;
    }

    s3
        .put_object()
        .bucket(&s3_bucket)
        .key("manifest.json")
        .body(serde_json::to_vec(&manifest).unwrap().into())
        .send()
        .await?;

    Ok(())
}
