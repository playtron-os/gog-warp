pub(crate) enum DownloadFileStatus {
    NotInitialized,
    Allocated,
    //Partial(u32), // Number of chunks that are downloaded
    Done,
}

pub enum DownloadState {
    Preparing,
    Allocating(f32),
    Downloading(DownloadProgress),
}

pub struct DownloadProgress {
    downloaded: u64,
    written: u64,
    size: u64,
    avg_network: f32,
    avg_disk: f32,
}
