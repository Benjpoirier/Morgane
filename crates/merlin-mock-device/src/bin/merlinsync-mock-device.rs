use std::collections::HashMap;
use std::path::PathBuf;

use merlin_mock_device::{FakeMerlinDevice, MockDeviceStore};

const REFERENCE_PLAYLIST: &[u8] = include_bytes!("../../assets/reference_playlist.bin");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut port: u16 = 50000;
    if let Some(index) = args.iter().position(|a| a == "--port")
        && let Some(value) = args.get(index + 1)
        && let Ok(parsed) = value.parse()
    {
        port = parsed;
    }

    let mut store_path: PathBuf = dirs::data_dir()
        .expect("répertoire Application Support introuvable")
        .join("merlinSyncMockDevice/store.sqlite3");
    if let Some(index) = args.iter().position(|a| a == "--store")
        && let Some(value) = args.get(index + 1)
    {
        store_path = PathBuf::from(value);
    }
    if args.iter().any(|a| a == "--fresh") {
        let _ = std::fs::remove_file(&store_path);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime tokio");

    runtime.block_on(async {
        let store = match MockDeviceStore::new(&store_path) {
            Ok(store) => store,
            Err(e) => {
                eprintln!(
                    "Impossible d'ouvrir le store {} : {e}",
                    store_path.display()
                );
                std::process::exit(1);
            }
        };
        let had_existing_content = !store.all().map(|f| f.is_empty()).unwrap_or(true);

        let mut seeded = false;
        if !had_existing_content && store.set("playlist.bin", REFERENCE_PLAYLIST).is_ok() {
            seeded = true;
        }

        let status_line = if had_existing_content {
            " Contenu precedent restaure depuis le store."
        } else if seeded {
            " Amorce avec une playlist d'exemple (Histoires/Documentaires/Calme/Derniers ajouts)."
        } else {
            " Aucune playlist.bin initiale (appareil \"vierge\")."
        };
        println!(
            "================================================\n \
             Faux appareil Merlin (dev/test, sans materiel)\n\
             ================================================\n \
             Port d'ecoute : {port}\n \
             Store persistant : {}\n\n \
             Dans l'app, onglet Connexion, l'un ou l'autre marche :\n   \
             - Hote : 192.168.4.1 (defaut deja bon, si `make mock-alias` a ete lance)\n   \
             - Hote : 127.0.0.1\n   \
             - Port : {port}\n\n\
             {status_line}\n \
             Validation firmware reelle activee (updatePlaylist genere un vrai\n \
             playlist.bin, verifie SHA-256 sur les envois).\n \
             Ctrl+C pour arreter.\n\
             ================================================",
            store_path.display()
        );

        match FakeMerlinDevice::start(port, HashMap::new(), true, Some(store)).await {
            Ok(_device) => {
                println!("[mock] en ecoute sur le port {port}...");

                tokio::signal::ctrl_c().await.expect("signal ctrl_c");
                println!("[mock] arret");
            }
            Err(e) => {
                eprintln!("Impossible de demarrer le faux appareil sur le port {port} : {e}");
                std::process::exit(1);
            }
        }
    });
}
