#[derive(Clone, Copy, Debug)]
pub enum ResizingSidebar {
    Left,
    Right,
}

#[derive(Clone, Debug, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    Available {
        version: String,
        download_url: String,
    },
    Updating,
    Error(String),
}
