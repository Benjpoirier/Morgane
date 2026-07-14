use std::collections::{HashMap, HashSet};
use std::time::Duration;

use merlin_domain::playlist::bin_parser;
use merlin_domain::playlist::model::{PlaylistFolder, PlaylistNode};
use merlin_domain::sync::device_file::DeviceFile;
use merlin_domain::sync::types::SyncError;
use merlin_protocol::client::{Frame, MerlinClient};
use merlin_protocol::commands;
use merlin_protocol::firmware_error_catalog;

use tracing::{debug, error};

use super::error::EngineError;
use super::session::{self, noop_log};

const INTER_FILE_DELAY: Duration = Duration::from_millis(80);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn fetch(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<Vec<PlaylistFolder>, EngineError> {
    let mut client = session::open(host, port, timeout, &noop_log).await?;

    let tree = match client.download_file("playlist.bin", DOWNLOAD_TIMEOUT).await {
        Ok(data) => {
            let tree = bin_parser::parse(&data);
            debug!(
                "playlist.bin : {} octets, {} enregistrements, {} dossiers racine",
                data.len(),
                data.len() / bin_parser::ACTUAL_RECORD_SIZE,
                tree.len(),
            );
            tree
        }
        Err(error) => {
            error!("playlist.bin illisible, arbre vide : {error}");
            Vec::new()
        }
    };
    client.close().await;
    Ok(tree)
}

pub async fn download_folder_images(
    host: &str,
    port: u16,
    folder_uuids: &[String],
    timeout: Duration,
) -> HashMap<String, Vec<u8>> {
    if folder_uuids.is_empty() {
        return HashMap::new();
    }
    let Ok(mut client) = session::open(host, port, timeout, &noop_log).await else {
        return HashMap::new();
    };
    let mut result = HashMap::new();
    for uuid in folder_uuids {
        if let Ok(data) = client
            .download_file(&format!("{uuid}.jpg"), DOWNLOAD_TIMEOUT)
            .await
            && !data.is_empty()
        {
            result.insert(uuid.clone(), data);
            tokio::time::sleep(INTER_FILE_DELAY).await;
        }
    }
    client.close().await;
    result
}

pub async fn list_files(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<Vec<DeviceFile>, EngineError> {
    let mut client = session::open(host, port, timeout, &noop_log).await?;
    let result = query_all_files(&mut client, timeout).await;
    client.close().await;
    result
}

pub async fn check_files_exist(
    host: &str,
    port: u16,
    names: &[String],
    timeout: Duration,
) -> Result<HashSet<String>, EngineError> {
    if names.is_empty() {
        return Ok(HashSet::new());
    }
    let mut client = session::open(host, port, timeout, &noop_log).await?;
    let result = async {
        let mut found = HashSet::new();
        for name in names {
            let frames = session::send(
                &mut client,
                "searchFile",
                &commands::search_file(name),
                timeout,
            )
            .await?;
            if frames
                .iter()
                .find(|f| f.opcode() == Some(commands::OP_SEARCH_FILE))
                .is_some_and(|f| f.body.len() > 1 && f.body[1] == 0)
            {
                found.insert(name.clone());
            }
        }
        Ok(found)
    }
    .await;
    client.close().await;
    result
}

pub async fn fetch_tree_and_check_integrity(
    host: &str,
    port: u16,
    timeout: Duration,
    on_progress: Option<IntegrityProgressFn<'_>>,
) -> Result<(Vec<PlaylistFolder>, HashSet<String>), EngineError> {
    let mut client = session::open(host, port, timeout, &noop_log).await?;
    let result = fetch_and_check_on(&mut client, timeout, on_progress).await;
    client.close().await;
    result
}

pub type IntegrityProgressFn<'a> = &'a (dyn Fn(usize, usize) + Send + Sync);

fn count_steps(folders: &[PlaylistFolder]) -> usize {
    let mut steps = 0;
    let mut stack: Vec<&PlaylistFolder> = folders.iter().filter(|f| !f.is_synthetic).collect();
    while let Some(folder) = stack.pop() {
        steps += 1;
        for child in &folder.children {
            match child {
                PlaylistNode::Sound { .. } => steps += 2,
                PlaylistNode::Folder(subfolder) => stack.push(subfolder),
            }
        }
    }
    steps
}

async fn fetch_and_check_on(
    client: &mut MerlinClient,
    timeout: Duration,
    on_progress: Option<IntegrityProgressFn<'_>>,
) -> Result<(Vec<PlaylistFolder>, HashSet<String>), EngineError> {
    let tree = match client.download_file("playlist.bin", DOWNLOAD_TIMEOUT).await {
        Ok(data) => bin_parser::parse(&data),

        Err(error) => {
            error!(
                "playlist.bin illisible, arbre vide et verification d'integrite sautee : {error}"
            );
            return Ok((Vec::new(), HashSet::new()));
        }
    };

    let total_steps = count_steps(&tree);
    let mut done_steps = 0usize;
    let step = |done: &mut usize| {
        *done += 1;
        if let Some(on_progress) = on_progress {
            on_progress(*done, total_steps);
        }
    };

    if let Some(on_progress) = on_progress {
        on_progress(0, total_steps);
    }
    debug!("controle d'integrite : {total_steps} etape(s)");

    let mut found: HashSet<String> = HashSet::new();

    async fn exists(
        client: &mut MerlinClient,
        name: &str,
        timeout: Duration,
    ) -> Result<bool, EngineError> {
        let frames =
            session::send(client, "searchFile", &commands::search_file(name), timeout).await?;
        Ok(frames
            .iter()
            .find(|f| f.opcode() == Some(commands::OP_SEARCH_FILE))
            .is_some_and(|f| f.body.len() > 1 && f.body[1] == 0))
    }

    let mut stack: Vec<&PlaylistFolder> = tree.iter().filter(|f| !f.is_synthetic).collect();
    while let Some(folder) = stack.pop() {
        let folder_jpg = format!("{}.jpg", folder.uuid);
        if exists(client, &folder_jpg, timeout).await? {
            found.insert(folder_jpg);
        }
        step(&mut done_steps);
        for child in &folder.children {
            match child {
                PlaylistNode::Sound { uuid, .. } => {
                    let mp3 = format!("{uuid}.mp3");
                    if exists(client, &mp3, timeout).await? {
                        found.insert(mp3);
                    } else {
                        let aac = format!("{uuid}.aac");
                        if exists(client, &aac, timeout).await? {
                            found.insert(aac);
                        }
                    }
                    step(&mut done_steps);
                    let jpg = format!("{uuid}.jpg");
                    if exists(client, &jpg, timeout).await? {
                        found.insert(jpg);
                    }
                    step(&mut done_steps);
                }
                PlaylistNode::Folder(subfolder) => stack.push(subfolder),
            }
        }
    }

    match query_all_files(client, timeout).await {
        Ok(files) => {
            for file in files.iter().filter(|f| f.size == 0) {
                if found.remove(&file.name) {
                    error!(
                        "{} : present mais VIDE (0 octet), signale comme manquant",
                        file.name
                    );
                }
            }
        }
        Err(error) => debug!("controle de taille saute (enumeration indisponible) : {error}"),
    }

    Ok((tree, found))
}

pub async fn query_all_files(
    client: &mut MerlinClient,
    timeout: Duration,
) -> Result<Vec<DeviceFile>, EngineError> {
    let is_count = |frames: &[Frame]| {
        frames
            .iter()
            .any(|f| f.opcode() == Some(commands::OP_GET_NUMBER_OF_FILES))
    };
    let count_frames = client
        .send_frame_command(&commands::get_number_of_files(), timeout, Some(&is_count))
        .await?;
    let Some(count_frame) = count_frames
        .iter()
        .find(|f| f.opcode() == Some(commands::OP_GET_NUMBER_OF_FILES))
        .filter(|f| f.body.len() >= 3)
    else {
        return Err(SyncError::NoResponse("getNumberOfFiles".to_string()).into());
    };
    let count = (count_frame.body[1] as usize) | ((count_frame.body[2] as usize) << 8);

    let mut files: Vec<DeviceFile> = Vec::with_capacity(count);
    let mut missing_indices: Vec<usize> = Vec::new();
    let is_info = |frames: &[Frame]| {
        frames
            .iter()
            .any(|f| f.opcode() == Some(commands::OP_GET_FILE_INFORMATION))
    };
    for index in 0..count {
        let frames = client
            .send_frame_command(
                &commands::get_file_information(index as u16),
                timeout,
                Some(&is_info),
            )
            .await?;
        let frame = frames
            .iter()
            .find(|f| f.opcode() == Some(commands::OP_GET_FILE_INFORMATION));
        let file = frame.and_then(|f| parse_file_information(&f.body));
        let Some(file) = file else {
            if let Some(status) = frame.and_then(|f| f.body.get(1)) {
                error!(
                    "getFileInformation({index}) : {}",
                    firmware_error_catalog::get_file_information_status(*status)
                );
            }

            missing_indices.push(index);
            continue;
        };
        files.push(file);

        tokio::time::sleep(INTER_FILE_DELAY).await;
    }
    if !missing_indices.is_empty() {
        return Err(SyncError::FileSearchFailed(format!(
            "enumeration incomplete : {}/{count} index sans reponse valide (ex: index {}) - la liste des fichiers reels ne peut pas etre garantie complete, operation annulee plutot que de risquer une perte de donnees silencieuse",
            missing_indices.len(),
            missing_indices[0]
        ))
        .into());
    }
    Ok(files)
}

fn parse_file_information(body: &[u8]) -> Option<DeviceFile> {
    if body.len() < 3 || body[1] != 0 {
        return None;
    }
    let name_length = body[2] as usize;
    let name_start = 3;
    let name_end = name_start + name_length;
    if body.len() < name_end + 4 {
        return None;
    }
    let name = String::from_utf8_lossy(&body[name_start..name_end]).into_owned();
    let size = u32::from_le_bytes(body[name_end..name_end + 4].try_into().ok()?) as usize;
    Some(DeviceFile::new(name, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(uuid: &str, children: Vec<PlaylistNode>) -> PlaylistFolder {
        let mut folder = PlaylistFolder::new(uuid, uuid);
        folder.children = children;
        folder
    }

    fn sound(uuid: &str) -> PlaylistNode {
        PlaylistNode::Sound {
            uuid: uuid.to_string(),
            title: uuid.to_string(),
        }
    }

    #[test]
    fn a_folder_costs_one_step_and_a_sound_costs_two() {
        let tree = vec![folder("cat", vec![sound("a"), sound("b")])];
        assert_eq!(count_steps(&tree), 1 + 2 * 2);
    }

    #[test]
    fn nested_folders_are_counted_recursively() {
        let tree = vec![folder(
            "cat",
            vec![PlaylistNode::Folder(folder("sub", vec![sound("a")]))],
        )];

        assert_eq!(count_steps(&tree), 4);
    }

    #[test]
    fn a_synthetic_folder_is_not_counted() {
        let mut synthetic = folder("orphans", vec![sound("a")]);
        synthetic.is_synthetic = true;
        assert_eq!(count_steps(&[synthetic]), 0);
    }

    #[test]
    fn an_empty_tree_costs_nothing() {
        assert_eq!(count_steps(&[]), 0);
    }
}
