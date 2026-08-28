//! Répondre à partir de documents, sans qu'ils quittent la machine.
//!
//! Le morph découpe un fichier, fait calculer les vecteurs de chaque morceau
//! par le moteur d'inférence local, et les range dans son propre index. Une
//! question suit le même chemin : elle est vectorisée, puis comparée aux
//! morceaux.
//!
//! **Ce morph ne rédige pas la réponse.** Il rend les passages retrouvés, avec
//! leur source et leur proximité ; c'est au modèle qui l'appelle de répondre à
//! partir d'eux. Un morph qui prétendrait rédiger inventerait la seule chose
//! qu'on lui demande de ne pas inventer.
//!
//! Le découpage, la sélection et le cosinus sont repris du socle, où ils
//! servaient déjà.

pub mod embed;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use embed::Role;

// ── Réglages ────────────────────────────────────────────────────────────────

/// Ce que le morph a besoin de savoir pour joindre le moteur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Adresse du serveur compatible OpenAI.
    #[serde(default = "endpoint_par_defaut")]
    pub endpoint: String,
    /// Modèle de plongement. Ce n'est pas le modèle de conversation : un
    /// serveur llama.cpp doit tourner avec `--embeddings`.
    #[serde(default = "modele_par_defaut")]
    pub embed_model: String,
    /// Taille d'un morceau, en caractères.
    #[serde(default = "taille_par_defaut")]
    pub chunk_size: usize,
    /// Recouvrement entre deux morceaux voisins.
    #[serde(default = "chevauchement_par_defaut")]
    pub chunk_overlap: usize,
    /// Cosinus minimum pour qu'un passage soit rendu. Zéro = pas de plancher.
    ///
    /// Le tri principal est *relatif* : il garde ce qui se détache du reste.
    /// Sur un corpus de deux ou trois morceaux, quelque chose se détache
    /// toujours, et une question hors sujet obtient alors un passage à faible
    /// score. Mesuré ici : 0,74 pour une question du corpus, 0,63 pour une
    /// question qui n'a rien à y voir — l'écart existe, mais il est mince.
    ///
    /// Le plancher reste à zéro par défaut : sa bonne valeur dépend du modèle
    /// de plongement, et la fixer au jugé écarterait des passages justes. Les
    /// scores étant rendus, l'appelant peut trancher lui-même.
    #[serde(default)]
    pub min_score: f32,
}

fn endpoint_par_defaut() -> String {
    "http://127.0.0.1:8080".into()
}
fn modele_par_defaut() -> String {
    "nomic-embed-text".into()
}
fn taille_par_defaut() -> usize {
    1200
}
fn chevauchement_par_defaut() -> usize {
    150
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: endpoint_par_defaut(),
            embed_model: modele_par_defaut(),
            chunk_size: taille_par_defaut(),
            chunk_overlap: chevauchement_par_defaut(),
            min_score: 0.0,
        }
    }
}

/// Lire les réglages. L'hôte désigne le fichier ; son absence n'est pas une
/// erreur, ce sont les valeurs par défaut.
pub fn config() -> Config {
    let Some(p) = std::env::var("LOCARYN_EXTENSION_CONFIG_FILE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
    else {
        return Config::default();
    };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ── Index ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Morceau {
    /// D'où vient ce passage — un chemin de fichier, en général.
    pub source: String,
    pub text: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Index {
    /// Le modèle qui a produit les vecteurs présents. Comparer des vecteurs
    /// venus de deux modèles différents ne veut rien dire.
    #[serde(default)]
    pub embed_model: String,
    #[serde(default)]
    pub chunks: Vec<Morceau>,
}

fn index_path() -> PathBuf {
    for key in ["LOCARYN_EXTENSION_DATA_DIR", "LOCARYN_MORPH_ROOT"] {
        if let Ok(dir) = std::env::var(key) {
            if !dir.trim().is_empty() {
                return PathBuf::from(dir).join("index.json");
            }
        }
    }
    PathBuf::from("index.json")
}

pub fn load_index() -> Result<Index, String> {
    let p = index_path();
    match std::fs::read_to_string(&p) {
        Ok(s) if s.trim().is_empty() => Ok(Index::default()),
        Ok(s) => {
            serde_json::from_str(&s).map_err(|e| format!("{} est illisible : {e}", p.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Index::default()),
        Err(e) => Err(format!("lecture de {} : {e}", p.display())),
    }
}

fn save_index(index: &Index) -> Result<(), String> {
    let p = index_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
    }
    let body = serde_json::to_string(index).map_err(|e| e.to_string())?;
    std::fs::write(&p, body).map_err(|e| format!("écriture de {} : {e}", p.display()))
}

// ── Indexation ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocumentRequest {
    pub file_path: String,
    #[serde(default)]
    pub chunk_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocumentResult {
    pub source: String,
    pub chunks_indexed: usize,
    pub embed_model: String,
    /// Taille des vecteurs. Utile pour voir d'un coup d'œil si le modèle a
    /// changé sous l'index.
    pub dim: usize,
    pub total_chunks: usize,
}

/// Indexer un fichier texte.
///
/// Réindexer la même source la remplace : sans cela, une correction laisserait
/// l'ancienne version dans l'index et les deux ressortiraient à la recherche.
pub async fn index_document(req: IndexDocumentRequest) -> Result<IndexDocumentResult, String> {
    let cfg = config();
    let chemin = PathBuf::from(&req.file_path);
    let texte = std::fs::read_to_string(&chemin)
        .map_err(|e| format!("Lecture de {} impossible : {e}", chemin.display()))?;
    if texte.trim().is_empty() {
        return Err(format!("{} est vide : rien à indexer.", chemin.display()));
    }

    let taille = req.chunk_size.unwrap_or(cfg.chunk_size);
    let morceaux = decouper(&texte, taille, cfg.chunk_overlap);
    if morceaux.is_empty() {
        return Err("Le découpage n'a produit aucun morceau.".into());
    }

    let client = reqwest::Client::new();
    let vecteurs = embed::embed(
        &cfg.endpoint,
        &client,
        &cfg.embed_model,
        &morceaux,
        Role::Document,
    )
    .await?;

    let mut index = load_index()?;
    // Un index ne mêle pas deux modèles : leurs vecteurs ne vivent pas dans le
    // même espace, et les comparer rendrait des voisins qui n'en sont pas.
    if !index.embed_model.is_empty() && index.embed_model != cfg.embed_model {
        return Err(format!(
            "L'index a été bâti avec « {} » et le réglage demande « {} ». Videz l'index \
             avant de changer de modèle.",
            index.embed_model, cfg.embed_model
        ));
    }
    index.embed_model = cfg.embed_model.clone();

    let source = chemin.display().to_string();
    index.chunks.retain(|c| c.source != source);
    let dim = vecteurs.first().map(|v| v.len()).unwrap_or(0);
    for (text, vector) in morceaux.into_iter().zip(vecteurs) {
        index.chunks.push(Morceau {
            source: source.clone(),
            text,
            vector,
        });
    }
    let indexes = index.chunks.iter().filter(|c| c.source == source).count();
    let total = index.chunks.len();
    save_index(&index)?;

    Ok(IndexDocumentResult {
        source,
        chunks_indexed: indexes,
        embed_model: cfg.embed_model,
        dim,
        total_chunks: total,
    })
}

// ── Recherche ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryRequest {
    pub query: String,
    #[serde(default)]
    pub top_k: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagChunkCitation {
    pub file_path: String,
    pub snippet: String,
    /// Cosinus entre la question et le morceau : 1 = même direction.
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryResponse {
    /// Les passages retrouvés. Vide quand rien ne se détache — c'est une
    /// réponse, pas un échec.
    pub citations: Vec<RagChunkCitation>,
    /// Ce que l'appelant doit savoir pour interpréter ce qu'il reçoit.
    pub note: String,
}

/// Retrouver les passages qui répondent à la question.
///
/// Ne rédige pas : rend les morceaux et laisse le modèle appelant répondre.
pub async fn answer_question(req: RagQueryRequest) -> Result<RagQueryResponse, String> {
    if req.query.trim().is_empty() {
        return Err("Question vide.".into());
    }
    let index = load_index()?;
    if index.chunks.is_empty() {
        return Ok(RagQueryResponse {
            citations: Vec::new(),
            note: "Aucun document n'est indexé. Passez d'abord par `index_document`.".into(),
        });
    }

    let cfg = config();
    if index.embed_model != cfg.embed_model {
        return Err(format!(
            "L'index a été bâti avec « {} » et le réglage demande « {} » : les vecteurs \
             ne sont pas comparables.",
            index.embed_model, cfg.embed_model
        ));
    }

    let client = reqwest::Client::new();
    let question = embed::embed(
        &cfg.endpoint,
        &client,
        &cfg.embed_model,
        &[req.query.clone()],
        Role::Question,
    )
    .await?
    .into_iter()
    .next()
    .ok_or("Le moteur n'a rendu aucun vecteur pour la question.")?;

    let mut hits: Vec<RagChunkCitation> = index
        .chunks
        .iter()
        .map(|c| RagChunkCitation {
            file_path: c.source.clone(),
            snippet: c.text.clone(),
            score: cosinus(&question, &c.vector),
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if cfg.min_score > 0.0 {
        hits.retain(|h| h.score >= cfg.min_score);
    }
    let scores: Vec<f32> = hits.iter().map(|h| h.score).collect();
    let gardes = retenir(&scores, req.top_k.unwrap_or(5));
    if gardes.is_empty() {
        return Ok(RagQueryResponse {
            citations: Vec::new(),
            note: "Aucun passage ne se détache : le corpus ne semble pas parler de cela. \
                   Mieux vaut le dire que de citer au hasard."
                .into(),
        });
    }
    let citations: Vec<RagChunkCitation> = gardes.into_iter().map(|i| hits[i].clone()).collect();
    let meilleur = citations.first().map(|c| c.score).unwrap_or(0.0);
    let note = format!(
        "{} passage(s) retrouvé(s), meilleur score {meilleur:.2}. Répondez à partir d'eux ; \
         ce morph ne rédige pas. Le tri est relatif : sur un petit corpus un passage ressort \
         toujours, donc un score bas veut dire que le corpus ne parle probablement pas du sujet.",
        citations.len()
    );
    Ok(RagQueryResponse { citations, note })
}

// ── État et remise à zéro ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RagStatus {
    pub chunks: usize,
    pub dim: usize,
    pub embed_model: String,
    pub sources: Vec<String>,
    pub endpoint: String,
}

pub fn rag_status() -> Result<RagStatus, String> {
    let index = load_index()?;
    let cfg = config();
    let mut sources: Vec<String> = index.chunks.iter().map(|c| c.source.clone()).collect();
    sources.sort();
    sources.dedup();
    Ok(RagStatus {
        chunks: index.chunks.len(),
        dim: index.chunks.first().map(|c| c.vector.len()).unwrap_or(0),
        embed_model: index.embed_model,
        sources,
        endpoint: cfg.endpoint,
    })
}

/// Retirer une source, ou tout l'index quand aucune n'est nommée.
pub fn clear_index(source: Option<&str>) -> Result<RagStatus, String> {
    let mut index = load_index()?;
    match source {
        Some(s) => index.chunks.retain(|c| c.source != s),
        None => {
            index.chunks.clear();
            index.embed_model.clear();
        }
    }
    save_index(&index)?;
    rag_status()
}

// ── Mesures et découpage, repris du socle ───────────────────────────────────

/// Le cosinus entre deux vecteurs : 1 = même direction, 0 = perpendiculaires.
pub fn cosinus(a: &[f32], b: &[f32]) -> f32 {
    let mut produit = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len().min(b.len()) {
        produit += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    produit / (na.sqrt() * nb.sqrt())
}

/// Lesquels des morceaux, triés par score décroissant, méritent d'être rendus.
///
/// Un corpus qui ne parle pas du sujet rend quand même un premier morceau : le
/// cosinus a toujours un maximum. Sans ce filtre, une question hors sujet
/// obtiendrait des citations sûres d'elles et fausses.
pub fn retenir(scores: &[f32], maximum: usize) -> Vec<usize> {
    if scores.is_empty() {
        return Vec::new();
    }
    let meilleur = scores[0];
    // Un score nul ou négatif ne ressemble à rien, quelle que soit l'échelle.
    if meilleur <= 0.0 {
        return Vec::new();
    }
    // Un seul morceau : aucune comparaison possible, donc aucune raison de le
    // refuser. Le modèle dira lui-même s'il ne répond pas.
    if scores.len() == 1 {
        return vec![0];
    }

    let moyenne_des_autres: f32 = scores[1..].iter().sum::<f32>() / (scores.len() - 1) as f32;
    let detachement = (meilleur - moyenne_des_autres) / meilleur;
    if detachement < 0.05 {
        return Vec::new();
    }

    // On garde le premier et ceux qui se détachent avec lui : au-dessus de la
    // moitié du chemin entre la moyenne et le meilleur.
    let seuil = moyenne_des_autres + (meilleur - moyenne_des_autres) * 0.5;
    scores
        .iter()
        .enumerate()
        .filter(|(_, s)| **s >= seuil)
        .map(|(i, _)| i)
        .take(maximum.max(1))
        .collect()
}

/// Découper un texte en morceaux qui tiennent dans une fenêtre de contexte.
///
/// La coupe suit les paragraphes tant qu'elle peut : couper au milieu d'une
/// phrase produit des morceaux dont ni l'un ni l'autre ne veut dire grand-chose.
/// Les morceaux se chevauchent un peu, parce qu'une réponse tombe souvent à
/// cheval sur une coupure, et qu'un chevauchement coûte moins cher qu'une
/// réponse manquée.
pub fn decouper(texte: &str, taille: usize, chevauchement: usize) -> Vec<String> {
    let taille = taille.max(200);
    let chevauchement = chevauchement.min(taille / 2);
    let mut morceaux = Vec::new();
    let mut courant = String::new();

    for paragraphe in texte.split("\n\n") {
        let p = paragraphe.trim();
        if p.is_empty() {
            continue;
        }
        // Un paragraphe plus long que la fenêtre est coupé net : il n'y a pas
        // de meilleure frontière à trouver dedans.
        if p.chars().count() > taille {
            if !courant.trim().is_empty() {
                morceaux.push(courant.trim().to_string());
                courant.clear();
            }
            let lettres: Vec<char> = p.chars().collect();
            let mut i = 0;
            while i < lettres.len() {
                let fin = (i + taille).min(lettres.len());
                morceaux.push(
                    lettres[i..fin]
                        .iter()
                        .collect::<String>()
                        .trim()
                        .to_string(),
                );
                if fin == lettres.len() {
                    break;
                }
                i = fin.saturating_sub(chevauchement);
            }
            continue;
        }
        if courant.chars().count() + p.chars().count() > taille && !courant.trim().is_empty() {
            morceaux.push(courant.trim().to_string());
            // Le chevauchement reprend la fin du morceau précédent.
            let queue: String = courant
                .chars()
                .rev()
                .take(chevauchement)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            courant = queue;
        }
        courant.push_str(p);
        courant.push_str("\n\n");
    }
    if !courant.trim().is_empty() {
        morceaux.push(courant.trim().to_string());
    }
    morceaux.retain(|m| !m.is_empty());
    morceaux
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn dossier(nom: &str) -> std::sync::MutexGuard<'static, ()> {
        let garde = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let d = std::env::temp_dir().join(format!("morph-rag-{nom}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::env::set_var("LOCARYN_EXTENSION_DATA_DIR", &d);
        std::env::remove_var("LOCARYN_EXTENSION_CONFIG_FILE");
        garde
    }

    #[test]
    fn le_cosinus_mesure_la_direction() {
        let a = [1.0f32, 0.0];
        assert!(
            (cosinus(&a, &[2.0, 0.0]) - 1.0).abs() < 1e-6,
            "même direction"
        );
        assert!(cosinus(&a, &[0.0, 1.0]).abs() < 1e-6, "perpendiculaires");
        assert!((cosinus(&a, &[-1.0, 0.0]) + 1.0).abs() < 1e-6, "opposés");
    }

    /// Le filtre existe pour ça : un corpus hors sujet rend quand même un
    /// premier morceau, puisque le cosinus a toujours un maximum.
    #[test]
    fn rien_n_est_retenu_quand_rien_ne_se_detache() {
        assert!(retenir(&[0.51, 0.50, 0.499, 0.498], 5).is_empty());
        assert_eq!(retenir(&[0.9, 0.2, 0.1], 5), vec![0]);
        assert!(retenir(&[-0.1, -0.2], 5).is_empty());
        assert_eq!(
            retenir(&[0.4], 5),
            vec![0],
            "un seul morceau passe toujours"
        );
    }

    #[test]
    fn le_decoupage_suit_les_paragraphes() {
        let t = "Un.\n\nDeux.\n\nTrois.";
        assert_eq!(decouper(t, 200, 20).len(), 1, "tout tient dans un morceau");
        let long = "a".repeat(900);
        let m = decouper(&long, 200, 20);
        assert!(
            m.len() > 3,
            "un paragraphe trop long est coupé : {}",
            m.len()
        );
        assert!(m.iter().all(|c| c.chars().count() <= 200));
    }

    #[test]
    fn un_index_absent_est_un_index_vide() {
        let _g = dossier("vide");
        let i = load_index().unwrap();
        assert!(i.chunks.is_empty());
        assert_eq!(rag_status().unwrap().chunks, 0);
    }

    #[test]
    fn une_question_sans_index_le_dit_sans_echouer() {
        let _g = dossier("sans-index");
        let r = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(answer_question(RagQueryRequest {
                query: "quoi ?".into(),
                top_k: None,
            }))
            .expect("ce n'est pas une erreur");
        assert!(r.citations.is_empty());
        assert!(r.note.contains("index_document"), "{}", r.note);
    }

    /// Changer de modèle sous un index existant rendrait des voisins qui n'en
    /// sont pas : les vecteurs ne vivent pas dans le même espace.
    #[test]
    fn un_index_d_un_autre_modele_est_refuse() {
        let _g = dossier("modele");
        let index = Index {
            embed_model: "un-autre-modele".into(),
            chunks: vec![Morceau {
                source: "a.md".into(),
                text: "x".into(),
                vector: vec![1.0, 0.0],
            }],
        };
        save_index(&index).unwrap();
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(answer_question(RagQueryRequest {
                query: "quoi ?".into(),
                top_k: None,
            }))
            .unwrap_err();
        assert!(err.contains("comparables"), "{err}");
    }

    #[test]
    fn vider_l_index_le_vide_vraiment() {
        let _g = dossier("vider");
        save_index(&Index {
            embed_model: "m".into(),
            chunks: vec![
                Morceau {
                    source: "a.md".into(),
                    text: "x".into(),
                    vector: vec![1.0],
                },
                Morceau {
                    source: "b.md".into(),
                    text: "y".into(),
                    vector: vec![1.0],
                },
            ],
        })
        .unwrap();
        let apres = clear_index(Some("a.md")).unwrap();
        assert_eq!(apres.chunks, 1);
        assert_eq!(apres.sources, vec!["b.md"]);
        assert_eq!(clear_index(None).unwrap().chunks, 0);
    }
}
