# S3 Sync

### Features
- Sync files to and from a single S3 bucket.
- Configure multiple directories to be synced to S3.
- Currently, the functionality is only provided via running the CLI manually. This works for my use case, but is definitely not ideal.
### Example Configuration
The TOML configuration location must either be `$S3_SYNC_CONFIG` or `$XDG_CONFIG_HOME/s3-sync/config.toml`. Below is a sample configuration file.
```toml
# Muliple paths are supported.
paths = [
  "/home/user/Documents/dir2",
  "/home/user/Documents/dir2",
]

[aws]
bucket = "aws-bucket-name"  # Only the name is needed. No S3 URI prefix required.
profile = "aws-profile"     # AWS profile name. If not specified, AWS credentials will be retrieved from the environment.
```
### Implementation Notes
- Currently the full path of system files are copied to S3. This means that syncing files between systems will only work as expected if the paths are the exact same in both systems.
- The logic for deciding between uploading versus downloading is the last modified date of the file. This is tracked in a `manifest.json` file in the root of the S3 bucket. Deleting or altering this manifest will produce unexpected results.

### Future
- The decision to either download or upload is done by comparing the last modified date of a file both locally and in S3. This is very quick to check, but a more robust implementation might also compare the file hashes.
- Need logic for handling deletion of files both locally and in S3. One example scenario is whether deleting a file locally should prevent it from being downloaded when the next sync is run.
- Add difference calculation.
- Add a service for automatically watching for file changes.
- Add support for changing configuration settings with CLI.
