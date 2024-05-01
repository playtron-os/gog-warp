use std::collections::HashMap;

use crate::content_system::types::{traits::FilePath, v1, v2, DepotEntry};

#[derive(Debug)]
pub enum ChangedEntry {
    ReDownload(DepotEntry),
    DiffPatch(DepotEntry),
    ChunkPatch((DepotEntry, DepotEntry)),
}

#[derive(Debug)]
pub struct DiffReport {
    product_id: String,
    new: Vec<DepotEntry>,
    deleted: Vec<DepotEntry>,
    changed: Vec<ChangedEntry>,
}

impl Default for DiffReport {
    fn default() -> Self {
        Self {
            product_id: String::default(),
            new: Vec::new(),
            deleted: Vec::new(),
            changed: Vec::new(),
        }
    }
}

fn map_entries(entries: Vec<DepotEntry>) -> HashMap<String, DepotEntry> {
    let mut map = HashMap::new();

    for entry in entries {
        map.insert(entry.path().to_lowercase(), entry);
    }

    map
}

fn chunk_diff(new_entry: &v2::DepotFile, old_entry: &v2::DepotFile) -> bool {
    new_entry
        .chunks()
        .iter()
        .any(|ch| old_entry.chunks().iter().any(|och| och.md5() == ch.md5()))
}

pub fn diff(
    product_id: &str,
    new_entries: Vec<DepotEntry>,
    old_entries: Vec<DepotEntry>,
    patches: Vec<DepotEntry>,
) -> DiffReport {
    let new = map_entries(new_entries);
    let old = map_entries(old_entries);
    let patches = map_entries(patches);

    let mut report = DiffReport::default();
    report.product_id = product_id.to_string();

    if old.len() == 0 {
        report.new = new.values().cloned().collect();
        return report;
    }

    for old_file in old.keys() {
        if new.get(old_file).is_none() {
            report.deleted.push(old.get(old_file).unwrap().clone());
        }
    }

    for (new_path, new_file) in new.iter() {
        let old_file = old.get(new_path);

        match (new_file, old_file) {
            // Any new file
            (file, None) => report.new.push(file.clone()),
            // Any directory
            (DepotEntry::V1(v1::DepotEntry::Directory(_d)), _) => {
                report.new.push(new_file.clone());
            }
            (DepotEntry::V2(v2::DepotEntry::Directory(_d)), _) => {
                report.new.push(new_file.clone());
            }
            (
                DepotEntry::V1(v1::DepotEntry::File(nf)),
                Some(DepotEntry::V1(v1::DepotEntry::File(of))),
            ) => {
                if nf.hash() != of.hash() {
                    report
                        .changed
                        .push(ChangedEntry::ReDownload(new_file.clone()))
                }
            }
            (
                DepotEntry::V2(v2::DepotEntry::File(nf)),
                Some(DepotEntry::V1(v1::DepotEntry::File(of))),
            ) => {
                // If file is empty treat is as new
                if nf.chunks().len() == 0 {
                    report.new.push(new_file.clone());
                    continue;
                }
                let new_checksum = nf
                    .md5()
                    .clone()
                    .unwrap_or_else(|| nf.chunks().first().unwrap().md5().to_owned());

                if new_checksum != *of.hash() {
                    report
                        .changed
                        .push(ChangedEntry::ReDownload(new_file.clone()));
                }
            }
            (
                DepotEntry::V1(v1::DepotEntry::File(nf)),
                Some(DepotEntry::V2(v2::DepotEntry::File(of))),
            ) => {
                let old_checksum = of
                    .md5()
                    .clone()
                    .unwrap_or_else(|| of.chunks().first().unwrap().md5().to_owned());

                if old_checksum != *nf.hash() {
                    report
                        .changed
                        .push(ChangedEntry::ReDownload(new_file.clone()));
                }
            }
            (
                DepotEntry::V2(v2::DepotEntry::File(nf)),
                Some(DepotEntry::V2(v2::DepotEntry::File(of))),
            ) => {
                // If file is empty treat is as new
                if nf.chunks().len() == 0 {
                    report.new.push(new_file.clone());
                    continue;
                }

                // If there was a patch for this path, use it
                if let Some(patch) = patches.get(new_path) {
                    report.changed.push(ChangedEntry::DiffPatch(patch.clone()));
                    continue;
                }

                // Re download file if there is only one chunk and it changed
                if nf.chunks().len() == 1 && of.chunks().len() == 1 {
                    if nf.chunks().first().unwrap().md5() != of.chunks().first().unwrap().md5() {
                        report
                            .changed
                            .push(ChangedEntry::ReDownload(new_file.clone()));
                        continue;
                    }
                }

                // If number of chunks changed
                if nf.chunks().len() != of.chunks().len() {
                    if chunk_diff(nf, of) {
                        report.changed.push(ChangedEntry::ChunkPatch((
                            new_file.clone(),
                            old_file.unwrap().clone(),
                        )));
                    } else {
                        report
                            .changed
                            .push(ChangedEntry::ReDownload(new_file.clone()));
                    }
                    continue;
                }

                // If sumarized checksum changed
                if (nf.md5().is_some() && of.md5().is_some() && nf.md5() != of.md5())
                    || (nf.sha256().is_some()
                        && of.sha256().is_some()
                        && nf.sha256() == of.sha256())
                {
                    if chunk_diff(nf, of) {
                        report.changed.push(ChangedEntry::ChunkPatch((
                            new_file.clone(),
                            old_file.unwrap().clone(),
                        )));
                    } else {
                        report
                            .changed
                            .push(ChangedEntry::ReDownload(new_file.clone()));
                    }
                }
            }

            _ => log::warn!("Unimplemented case matched {:?} {:?}", new_file, old_file),
        }
    }

    report
}
