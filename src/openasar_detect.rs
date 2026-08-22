use std::path::PathBuf;

pub fn detect(resources: &PathBuf) -> bool {

    let marker = resources.join("openasar.lock");

    marker.exists()

}