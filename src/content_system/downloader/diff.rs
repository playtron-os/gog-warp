use std::collections::{HashMap, HashSet};

use crate::content_system::types::{traits::FilePath, v1, v2, DepotEntry, FileList};

#[derive(Debug, Default)]
pub struct DiffReport {
    pub(crate) download: Vec<FileList>,
    pub(crate) directories: Vec<DepotEntry>,
    pub(crate) deleted: Vec<DepotEntry>,
    size_diff: i64,
}

fn map_list(lists: &Vec<FileList>) -> HashMap<String, &DepotEntry> {
    let mut map = HashMap::new();
    for list in lists {
        for entry in &list.files {
            map.insert(entry.path().to_lowercase(), entry);
        }
    }
    map
}

pub fn diff(
    new_entries: Vec<FileList>,
    old_entries: Vec<FileList>,
    patches: Vec<FileList>,
) -> DiffReport {
    let new = map_list(&new_entries);
    let old = map_list(&old_entries);
    let patches_set = map_list(&patches);

    let mut deleted_paths: HashSet<String> = HashSet::new();
    let mut final_download: HashSet<String> = HashSet::from_iter(new.keys().cloned());

    // Prepare report
    let mut report = DiffReport::default();

    for patch in patches_set.keys() {
        final_download.remove(patch);
    }

    for old_file in old.keys() {
        if !new.contains_key(old_file) {
            deleted_paths.insert(old.get(old_file).unwrap().path().to_lowercase());
        }
    }

    for (new_path, new_file) in new.iter() {
        let old_file = old.get(new_path);

        match (new_file, old_file) {
            // Any directory
            (DepotEntry::V1(v1::DepotEntry::Directory(_d)), _) => {
                report.directories.push((*new_file).clone())
            }
            (DepotEntry::V2(v2::DepotEntry::Directory(_d)), _) => {
                report.directories.push((*new_file).clone())
            }
            // Any new file
            (_file, None) => (),
            (
                DepotEntry::V1(v1::DepotEntry::File(nf)),
                Some(DepotEntry::V1(v1::DepotEntry::File(of))),
            ) => {
                if nf.hash() == of.hash() {
                    final_download.remove(&nf.path().to_lowercase());
                }
            }
            (
                DepotEntry::V2(v2::DepotEntry::File(nf)),
                Some(DepotEntry::V1(v1::DepotEntry::File(of))),
            ) => {
                // If file is empty treat is as new
                if nf.chunks().is_empty() {
                    continue;
                }
                let new_checksum = nf
                    .md5()
                    .clone()
                    .unwrap_or_else(|| nf.chunks().first().unwrap().md5().to_owned());

                if new_checksum == *of.hash() {
                    final_download.remove(&nf.path().to_lowercase());
                }
            }
            (
                DepotEntry::V1(v1::DepotEntry::File(nf)),
                Some(DepotEntry::V2(v2::DepotEntry::File(of))),
            ) => {
                if *nf.size() == 0 {
                    continue;
                }
                let old_checksum = of
                    .md5()
                    .clone()
                    .unwrap_or_else(|| of.chunks().first().unwrap().md5().to_owned());

                if old_checksum == *nf.hash() {
                    final_download.remove(&nf.path().to_lowercase());
                }
            }
            (
                DepotEntry::V2(v2::DepotEntry::File(nf)),
                Some(DepotEntry::V2(v2::DepotEntry::File(of))),
            ) => {
                // If file is empty treat is as new
                if nf.chunks().is_empty() {
                    continue;
                }

                // If there was a patch for this path, skip it
                if patches_set.contains_key(new_path) {
                    continue;
                }

                // Re download file if there is only one chunk and it changed
                if nf.chunks().len() == 1
                    && of.chunks().len() == 1
                    && nf.chunks().first().unwrap().md5() == of.chunks().first().unwrap().md5()
                {
                    final_download.remove(&nf.path().to_lowercase());
                    continue;
                }

                // If sumarized checksum changed
                if (nf.md5().is_some() && of.md5().is_some() && nf.md5() == of.md5())
                    || (nf.sha256().is_some()
                        && of.sha256().is_some()
                        && nf.sha256() == of.sha256())
                {
                    final_download.remove(&nf.path().to_lowercase());
                }
            }

            _ => log::warn!("Unimplemented case matched {:?} {:?}", new_file, old_file),
        }
    }

    drop(new);

    for file_list in new_entries {
        let mut new_list = FileList::new(file_list.product_id, Vec::new());
        let mut needs_sfc = false;

        for entry in file_list.files {
            if final_download.contains(&entry.path().to_lowercase()) {
                if !needs_sfc {
                    if let DepotEntry::V2(v2::DepotEntry::File(file)) = &entry {
                        needs_sfc = file.sfc_ref().is_some()
                    }
                }
                new_list.files.push(entry);
            }
        }
        if needs_sfc {
            new_list.sfc = file_list.sfc;
        }
        if !new_list.files.is_empty() {
            report.download.push(new_list)
        }
    }

    // Track down deleted files
    for file_list in old_entries {
        for entry in file_list.files {
            if deleted_paths.contains(&entry.path().to_lowercase()) {
                report.deleted.push(entry);
            }
        }
    }

    report.download.extend(patches);

    report
}
