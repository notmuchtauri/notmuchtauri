use async_imap::extensions::idle::IdleResponse::*; // <-- IMPORT CRUCIAL
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;
use tokio::time;
use async_imap::extensions::idle::IdleResponse::*;
use futures::StreamExt;

use async_std::{net::TcpStream, task, task::sleep};

#[derive(Debug, Clone)]
struct ImapAccount {
    name: String,
    host: String,
    user: String,
    pass_cmd: Option<String>,
    pass: Option<String>,
}

// ---------------------------------------------------------
// PARSING DE LA CONFIGURATION (~/.mbsyncrc)
// ---------------------------------------------------------

fn parse_mbsyncrc() -> Vec<ImapAccount> {
    let mut accounts = Vec::new();
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => return accounts,
    };
    
    let mbsyncrc_path = home_dir.join(".mbsyncrc");
    let file = match File::open(&mbsyncrc_path) {
        Ok(f) => f,
        Err(_) => return accounts,
    };

    let reader = BufReader::new(file);
    let mut current_account: Option<ImapAccount> = None;
    let mut is_imaps = false;

    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() < 2 { continue; }
        
        let key = parts[0];
        let val = parts[1].trim().trim_matches('"');

        match key {
            "IMAPAccount" => {
                if let Some(acc) = current_account.take() {
                    if is_imaps { accounts.push(acc); }
                }
                current_account = Some(ImapAccount {
                    name: val.to_string(),
                    host: String::new(),
                    user: String::new(),
                    pass_cmd: None,
                    pass: None,
                });
                is_imaps = false;
            }
            "Host" => if let Some(ref mut acc) = current_account { acc.host = val.to_string(); },
            "User" => if let Some(ref mut acc) = current_account { acc.user = val.to_string(); },
            "PassCmd" => if let Some(ref mut acc) = current_account { acc.pass_cmd = Some(val.to_string()); },
            "Pass" => if let Some(ref mut acc) = current_account { acc.pass = Some(val.to_string()); },
            "SSLType" => if val == "IMAPS" { is_imaps = true; },
            _ => {}
        }
    }

    if let Some(acc) = current_account {
        if is_imaps { accounts.push(acc); }
    }
    accounts
}

// ---------------------------------------------------------
// FONCTIONS ASYNCHRONES (MOT DE PASSE ET SYNCHRO)
// ---------------------------------------------------------

async fn get_password(account: &ImapAccount) -> Result<String, String> {
    if let Some(pass) = &account.pass {
        return Ok(pass.clone());
    }

    if let Some(pass_cmd) = &account.pass_cmd {
        let output = Command::new("sh")
            .arg("-c")
            .arg(pass_cmd)
            .output()
            .await
            .map_err(|e| format!("Erreur exécution shell: {}", e))?;

        if output.status.success() {
            let pass = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(pass);
        } else {
            return Err("PassCmd a échoué.".into());
        }
    }

    Err("Aucune directive Pass ou PassCmd configurée.".into())
}

async fn trigger_sync(account_name: &str, app_handle: &AppHandle) {
    println!("[IDLE: {}] ⚡ Lancement de la synchronisation mbsync + notmuch...", account_name);
    
    let sync_cmd = "sem --fg -j 1 --id mbsync '/usr/bin/mbsync -a' ; sem -j 1 --fg --id notmuch '/usr/bin/notmuch new --quiet'";
    
    let status = Command::new("sh")
        .arg("-c")
        .arg(sync_cmd)
        .status()
        .await;

    if let Ok(s) = status {
        if s.success() {
            println!("[IDLE: {}] ✅ Synchro terminée. Émission de l'événement Vue.js.", account_name);
            let _ = app_handle.emit("mail-synced", account_name);
        } else {
            eprintln!("[IDLE: {}] ❌ Échec de la commande de synchronisation.", account_name);
        }
    }
}

// ---------------------------------------------------------
// LE WORKER IDLE PRINCIPAL
// ---------------------------------------------------------

async fn start_idle_worker(account: ImapAccount, app_handle: AppHandle) {
    let acc_name = account.name.clone();
    println!("[IDLE: {}] 🚀 Démarrage du worker (Tokio Task) avec async-imap 0.11", acc_name);

    loop {
        println!("[IDLE: {}] 🔄 Connexion en cours...", acc_name);

        let password = match get_password(&account).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[IDLE: {}] ❌ Erreur mdp: {}. Pause 60s.", acc_name, e);
                time::sleep(Duration::from_secs(60)).await;
                continue;
            }
        };

        // =========================================================
        // CONNEXION TCP + TLS (Séquence manuelle stricte)
        // =========================================================
        let imap_addr = (account.host.as_str(), 993);
        
        // 1. Établissement du tunnel TCP pur
        println!("[IDLE: {}] 🌐 Connexion TCP à {}:993...", acc_name, account.host);
        let tcp_stream = match TcpStream::connect(imap_addr).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[IDLE: {}] ❌ Erreur connexion TCP: {}. Pause 30s.", acc_name, e);
                time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        // 2. Négociation TLS par-dessus le tunnel TCP
        println!("[IDLE: {}] 🔒 Négociation de la couche TLS...", acc_name);
        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = match tls.connect(&account.host, tcp_stream).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[IDLE: {}] ❌ Erreur handshake TLS: {}. Pause 30s.", acc_name, e);
                time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        // 3. Création du client IMAP
        let client = async_imap::Client::new(tls_stream);

        // =========================================================
        // LOGIN ET SÉLECTION DU DOSSIER
        // =========================================================
        println!("[IDLE: {}] 🔐 Tentative de Login...", acc_name);
        let mut session = match client.login(&account.user, &password).await {
            Ok(s) => {
                println!("[IDLE: {}] ✅ Login réussi.", acc_name);
                s
            },
            Err((e, _unauth_client)) => {
                eprintln!("[IDLE: {}] ❌ Erreur Login: {}. Pause 60s.", acc_name, e);
                time::sleep(Duration::from_secs(60)).await;
                continue;
            }
        };

        if let Err(e) = session.select("INBOX").await {
            eprintln!("[IDLE: {}] ❌ Erreur sélection INBOX: {}. Pause 30s.", acc_name, e);
            let _ = session.logout().await;
            time::sleep(Duration::from_secs(30)).await;
            continue;
        }

        // =========================================================
        // BOUCLE D'IDLE (Écoute continue)
        // =========================================================
        loop {
            println!("[IDLE: {}] -- initializing idle", acc_name);
            let mut idle = session.idle();
            
            if let Err(e) = idle.init().await {
                eprintln!("[IDLE: {}] ❌ Erreur lors de l'initialisation IDLE: {}", acc_name, e);
                break; // Force la reconnexion globale
            }

            println!("[IDLE: {}] -- idle async wait", acc_name);
            let (idle_wait, _interrupt) = idle.wait();

            // On englobe l'attente dans un timeout absolu (35 min) par sécurité réseau
            let idle_result = match time::timeout(Duration::from_secs(35 * 60), idle_wait).await {
                Ok(Ok(res)) => res,
                Ok(Err(e)) => {
                    eprintln!("[IDLE: {}] ❌ Erreur de flux pendant IDLE: {}", acc_name, e);
                    break;
                }
                Err(_) => {
                    eprintln!("[IDLE: {}] ⏲️ Timeout réseau (35 min). Reconnexion TCP...", acc_name);
                    break;
                }
            };

            // Analyse de la réponse exacte de l'API
            match idle_result {
                ManualInterrupt => {
                    println!("[IDLE: {}] -- IDLE manually interrupted", acc_name);
                    session = match idle.done().await {
                        Ok(s) => s,
                        Err(_) => break,
                    };
                }
                Timeout => {
                    println!("[IDLE: {}] -- IDLE timed out (KeepAlive 29m). Envoi du DONE pour reset.", acc_name);
                    session = match idle.done().await {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[IDLE: {}] ❌ Erreur lors du DONE: {}", acc_name, e);
                            break;
                        }
                    };
                }
                NewData(data) => {
                    let s = String::from_utf8_lossy(data.borrow_owner());
                    println!("[IDLE: {}] -- IDLE data reçue:\n{}", acc_name, s.trim());
                    
                    // ON SORT DE L'IDLE
                    session = match idle.done().await {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[IDLE: {}] ❌ Erreur lors du DONE après data: {}", acc_name, e);
                            break;
                        }
                    };

                    // ON SYNCHRONISE
                    trigger_sync(&acc_name, &app_handle).await;
                }
            }
        }

        println!("[IDLE: {}] -- logging out", acc_name);
        // let _ = session.logout().await;
        
        println!("[IDLE: {}] 💤 Pause 10s avant reconnexion globale...", acc_name);
        time::sleep(Duration::from_secs(10)).await;
    }
}


// ---------------------------------------------------------
// POINT D'ENTRÉE TAUXI
// ---------------------------------------------------------

pub fn start_imap_idle_daemons(app_handle: AppHandle) {
    let accounts = parse_mbsyncrc();
    println!("[IDLE-MANAGER] Lancement des tâches Tokio pour {} comptes.", accounts.len());

    for account in accounts {
        let handle_clone = app_handle.clone();
        
        tauri::async_runtime::spawn(async move {
            start_idle_worker(account, handle_clone).await;
        });
    }
}