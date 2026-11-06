use std::path::Path;

use crate::core::update::Download;
use crate::error::UpdateError;

pub fn replace_executable(executable: &Path, _download: &Download) -> Result<(), UpdateError> {
    Err(UpdateError::Protected {
        path: executable.to_owned(),
    })
}
