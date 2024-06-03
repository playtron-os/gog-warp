pub(crate) enum DownloadFileStatus {
    NotInitialized,
    Allocated,
    //Partial(u32), // Number of chunks that are downloaded
    PatchDownloaded,
    Done,
}

pub enum DownloadState {
    Preparing,
    Allocating(f32),
    Downloading(DownloadProgress),
}

pub struct DownloadProgress {
    pub downloaded: u64,
    pub written: u64,
    pub total_download: u64,
    pub total_size: u64,
    pub avg_network: f32,
    pub avg_disk: f32,
}
