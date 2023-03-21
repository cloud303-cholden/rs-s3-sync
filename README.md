# Obsidian S3 Sync

### Features
- Upload Markdown files from a single configured vault to an S3 bucket.
- Download Markdown files from an S3 bucket to a configured vault.

### Future
- The decision to either download or upload is done by comparing the last modified date of a file both locally and in S3. This decision should also be based on whether or not there are any changes.
- Need logic for handling deletion of files both locally and in S3. One example scenario is whether deleting a file locally should prevent it from being downloaded when the next sync is run.
- Add support for multiple vaults.
- Add difference calculation.
- Add a service for automatically watching for file changes.
- Add support for changing configuration settings with CLI.
