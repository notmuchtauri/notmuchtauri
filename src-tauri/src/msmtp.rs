use lettre::message::{Attachment, Message, MultiPart, SinglePart};
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::interval;

use crate::notmuch::NotMuchWrapper;

// --- RAW NOTMUCH MODELS ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPayload {
    pub path: String,              // Chemin absolu vers le fichier
    pub mime_type: Option<String>, // Optionnel: ex "application/pdf"
    pub is_part: bool,
    pub part_id: i16,
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailPayload {
    pub from: String, // Adresse d'expédition
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub is_html: bool,
    pub attachments: Vec<AttachmentPayload>,
    pub account: Option<String>, // Optionnel: pour passer `-a <compte>` à msmtp
    pub sent_folder: String,
}

pub struct MSMTPWrapper;

impl MSMTPWrapper {
    pub fn check_installation() -> bool {
        Command::new("msmtp").arg("--version").output().is_ok()
    }

    /// Tente d'envoyer un e-mail brut via msmtp. Retourne true si succès.
    fn send_raw_email(raw_email: &[u8], account: Option<String>) -> bool {
        let mut cmd = Command::new("msmtp");
        cmd.arg("-t");

        if let Some(account) = account {
            cmd.arg("-a").arg(account);
        }

        let mut child = match cmd // Demande à msmtp de lire le 'To:' et 'From:' dans les headers
            .stdin(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        if let Some(mut stdin) = child.stdin.take() {
            if stdin.write_all(raw_email).is_err() {
                return false;
            }
        }

        match child.wait_with_output() {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// Sauvegarde un e-mail brut dans un dossier spécifique via notmuch
    fn insert_to_notmuch(raw_email: &[u8], folder: &str, extra_tags: &[&str]) -> bool {
        let mut args = vec![
            "insert".to_string(),
            format!("--folder={}", folder),
            "--create-folder".to_string(),
        ];
        for tag in extra_tags {
            args.push(tag.to_string());
        }

        let mut child = match Command::new("notmuch")
            .args(&args)
            .stdin(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(raw_email);
        }

        child.wait().map(|s| s.success()).unwrap_or(false)
    }
    pub async fn send_email(payload: EmailPayload) -> Result<(), String> {
        // 1. Initialisation du constructeur de mail
        let mut builder = Message::builder().subject(payload.subject);

        if payload.from != "" {
            builder = builder.from(
                payload
                    .from
                    .parse()
                    .map_err(|e| format!("'From' invalide: {}", e))?,
            )
        }

        for addr in payload.to {
            builder = builder.to(addr.parse().map_err(|e| format!("'To' invalide: {}", e))?);
        }
        for addr in payload.cc {
            builder = builder.cc(addr.parse().map_err(|e| format!("'Cc' invalide: {}", e))?);
        }
        for addr in payload.bcc {
            builder = builder.bcc(addr.parse().map_err(|e| format!("'Bcc' invalide: {}", e))?);
        }

        // 2. Construction du corps (HTML ou texte brut)
        let body_part = if payload.is_html {
            SinglePart::html(payload.body)
        } else {
            SinglePart::plain(payload.body)
        };

        let mut multipart = MultiPart::mixed().singlepart(body_part);

        // 3. Ajout des pièces jointes
        for att in payload.attachments {
            let path = Path::new(&att.path);
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("fichier_joint")
                .to_string();

            // On utilise std::fs directement en Rust. Cela contourne les restrictions
            // de sécurité du frontend (Tauri fs scope), ce qui est idéal ici.
            if !att.is_part {
                let content = fs::read(path)
                    .map_err(|e| format!("Erreur de lecture de la PJ ({}): {}", file_name, e))?;

                let mime_type = att
                    .mime_type
                    .unwrap_or_else(|| "application/octet-stream".to_string())
                    .parse()
                    .map_err(|_| format!("Type MIME invalide pour {}", file_name))?;

                let attachment = Attachment::new(file_name).body(content, mime_type);
                multipart = multipart.singlepart(attachment);
            } else {
                let output = Command::new("notmuch")
                    .args([
                        "show",
                        "--format=raw",
                        &format!("--part={}", att.part_id),
                        &format!("id:{}", att.message_id),
                    ])
                    .output()
                    .map_err(|e| format!("Erreur lors de l'appel à notmuch: {}", e))?;

                if output.status.success() {
                    let content = output.stdout;
                    let mime_type = att
                        .mime_type
                        .unwrap_or_else(|| "application/octet-stream".to_string())
                        .parse()
                        .map_err(|_| format!("Type MIME invalide pour {}", file_name))?;

                    let attachment = Attachment::new(file_name).body(content, mime_type);
                    multipart = multipart.singlepart(attachment);
                }
            }
        }

        // 4. Génération du message MIME brut
        let email = builder
            .multipart(multipart)
            .map_err(|e| format!("Erreur de construction du mail: {}", e))?;

        let email_bytes = email.formatted();

        let sent_successfully = MSMTPWrapper::send_raw_email(&email_bytes, payload.account);

        if sent_successfully {
            // 2a. Succès : Sauvegarde dans Sent
            MSMTPWrapper::insert_to_notmuch(
                &email_bytes,
                &payload.sent_folder,
                &["+sent", "-unread", "-new"],
            );

            println!("Email envoyé avec succès");
        } else {
            // 2b. Échec (pas de réseau) : Sauvegarde dans Outbox
            MSMTPWrapper::insert_to_notmuch(
                &email_bytes,
                "Outbox",
                &["+outbox", "-unread", "-new"],
            );
            // On retourne quand même "Ok" au frontend pour ne pas bloquer l'UI,
            // mais on prévient que c'est dans la boîte d'envoi.
            println!("Réseau indisponible. L'e-mail a été placé dans la boîte d'envoi (Outbox).")
        }

        Ok(())
    }

    pub async fn send_ics_email(
        to_addresses: Vec<String>,
        subject: &str,
        body: &str,
        ics_content: &str,
        sentfolder: &str,
    ) -> Result<(), String> {
        // 1. Démarrer la construction de l'e-mail
        // Pensez à remplacer "votre_email" par la lecture de l'adresse de l'utilisateur
        let mut builder = Message::builder()
            .from("votre_email@domaine.com".parse().unwrap())
            .subject(subject);

        // Ajouter tous les destinataires (lettre gère "Nom <email@domaine.com>" ou juste "email@domaine.com")
        for addr in to_addresses {
            let parsed_addr = addr
                .parse()
                .map_err(|_| format!("Adresse invalide: {}", addr))?;
            builder = builder.to(parsed_addr);
        }

        // 2. Créer l'e-mail Multipart (Texte + Fichier ICS)
        // Le Content-Type text/calendar; method=REQUEST permet aux clients mail (Outlook/Gmail)
        // d'afficher directement les boutons "Accepter" ou "Refuser".
        let email = builder
            .multipart(
                MultiPart::mixed()
                    .singlepart(SinglePart::plain(body.to_string()))
                    .singlepart(Attachment::new(String::from("invite.ics")).body(
                        ics_content.to_string(),
                        "text/calendar; method=REQUEST".parse().unwrap(),
                    )),
            )
            .map_err(|e| format!("Erreur de construction de l'email: {}", e))?;

        let email_bytes = email.formatted();
        let sent_successfully = MSMTPWrapper::send_raw_email(&email_bytes, None);
        if sent_successfully {
            // 2a. Succès : Sauvegarde dans Sent
            MSMTPWrapper::insert_to_notmuch(
                &email_bytes,
                sentfolder,
                &["+sent", "-unread", "-new"],
            );

            println!("Email envoyé avec succès");
        } else {
            // 2b. Échec (pas de réseau) : Sauvegarde dans Outbox
            MSMTPWrapper::insert_to_notmuch(
                &email_bytes,
                sentfolder,
                &["+outbox", "-unread", "-sent", "-new"],
            );
            // On retourne quand même "Ok" au frontend pour ne pas bloquer l'UI,
            // mais on prévient que c'est dans la boîte d'envoi.
            println!("Réseau indisponible. L'e-mail a été placé dans la boîte d'envoi (Outbox).")
        }

        Ok(())
    }

    pub async fn process_outbox_daemon() {
        // Vérifie la boîte d'envoi toutes les 5 minutes
        let mut ticker = interval(Duration::from_secs(5 * 60));

        loop {
            ticker.tick().await;

            // 1. Chercher les IDs des messages ayant le tag outbox (Asynchrone)
            let search_output = match tokio::process::Command::new("notmuch")
                .args(["search", "--output=messages", "tag:outbox"])
                .output()
                .await
            {
                Ok(out) => out,
                Err(e) => {
                    eprintln!("Daemon Outbox: Erreur lors de la recherche notmuch - {}", e);
                    continue;
                }
            };

            let msgs_str = String::from_utf8_lossy(&search_output.stdout);

            for msg_id_raw in msgs_str.lines() {
                let msg_id = msg_id_raw.trim();
                if msg_id.is_empty() {
                    continue;
                }

                println!("Daemon Outbox: Tentative d'envoi pour {}", msg_id);

                // 2. Extraire le contenu brut du message (Asynchrone)
                let raw_output = match tokio::process::Command::new("notmuch")
                    .args(["show", "--format=raw", msg_id])
                    .output()
                    .await
                {
                    Ok(out) if !out.stdout.is_empty() => out,
                    _ => {
                        eprintln!(
                            "Daemon Outbox: Impossible d'extraire le contenu de {}",
                            msg_id
                        );
                        continue;
                    }
                };

                // 3. Tenter l'envoi
                // Si votre méthode `send_raw_email` utilise std::process::Command (synchrone),
                // il FAUT l'exécuter dans un thread de blocage pour ne pas figer Tauri.
                let raw_bytes = raw_output.stdout.clone();
                let send_success = tokio::task::spawn_blocking(move || {
                    MSMTPWrapper::send_raw_email(&raw_bytes, None)
                })
                .await
                .unwrap_or(false);

                if send_success {
                    println!("Daemon Outbox: Succès ! Email {} expédié.", msg_id);

                    // 4. Mettre à jour les tags (Idem, on encapsule dans spawn_blocking si vos wrappers sont synchrones)
                    let msg_id_clone = msg_id.to_string();
                    tokio::task::spawn_blocking(move || {
                        // Nettoie l'ID de potentiels préfixes de notmuch si nécessaire
                        NotMuchWrapper::modify_message_tag(&msg_id_clone, "outbox", "remove");
                        NotMuchWrapper::modify_message_tag(&msg_id_clone, "sent", "add");
                    })
                    .await
                    .unwrap_or(());
                } else {
                    eprintln!(
                        "Daemon Outbox: Échec de l'envoi pour {}. Nouvel essai au prochain cycle.",
                        msg_id
                    );
                }
            }
        }
    }
}
