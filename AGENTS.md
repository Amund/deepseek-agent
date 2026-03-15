# AGENTS.md - deepseek-agent

Documentation pour les agents IA reprenant le projet. Ce fichier contient le contexte, l'état actuel, les décisions techniques et les prochaines étapes.

## 🎯 Contexte du Projet

**Nom** : deepseek-agent  
**Langage** : Rust  
**Objectif** : Un agent CLI minimal utilisant l'API DeepSeek avec un seul outil - l'exécution de commandes shell (`sh`).  
**Philosophie** : Léger, sans persistance (tout en mémoire), destiné à tourner dans un conteneur Docker, donc sans sandboxing supplémentaire pour les commandes exécutées.

## 📁 Structure du Projet (mise à jour)

```
deepseek-agent/
├── src/
│   └── main.rs            # Code principal
├── Cargo.toml             # Configuration Rust
├── README.md              # Documentation complétée
├── AGENTS.md              # Ce fichier
├── env.example            # Template de configuration
├── .gitignore             # Fichiers ignorés
└── examples/              # Exemples d'utilisation
    └── basic_usage.md     # Exemples détaillés
```

## 🔍 Analyse Technique

### Problèmes Identifiés (src/main.rs) - État actuel

1. **✅ Fichier Cargo.toml créé** - Projet maintenant compilable
2. **✅ Sécurité améliorée** (ligne ~154) :
   ```rust
   let cmd_name = command.split_whitespace().next().unwrap_or("");
   ```
   → Validation étendue : blacklist sur tous les tokens, patterns dangereux détectés, validation basique des séparateurs
3. **✅ Gestion optimisée des tool_calls** (lignes 135-140) :
   ```rust
   for tool_call in tool_calls {
       // ... exécution
   }
   // Un seul appel API final après traitement de tous les tool_calls
   ```
   → Collecte de tous les résultats d'abord, puis un seul appel API final
4. **✅ Historique limité** :
   ```rust
   messages: Vec<Message>  // Avec fenêtre glissante configurable
   ```
   → Limite configurable de messages/tokens, conservation du message système, optimisation du cache KV
5. **✅ Gestion d'erreurs avancée** - Retries avec backoff exponentiel, timeout shell, messages d'erreur améliorés
6. **✅ Warnings nettoyés** - Import inutilisé `VecDeque` supprimé, champ `finish_reason` supprimé
7. **✅ Chargement automatique des fichiers de contexte** - AGENTS.md et README.md chargés automatiquement au démarrage (configurable via `DEEPSEEK_AGENT_SKIP_CONTEXT_FILES`)
8. **✅ Streaming des réponses** - Implémentation du streaming SSE avec gestion de buffer pour les chunks fragmentés, configurable via `DEEPSEEK_AGENT_STREAM`

## 🛠️ Décisions Techniques Prises

### 1. Architecture
- **Pas de Docker pour le moment** : L'utilisateur préfère compiler et exécuter en local
- **Priorité** : Rendre le projet compilable avant toute optimisation

## 📋 État d'Avancement (mise à jour)

### ✅ Tâches Complétées
- [x] Analyse du code existant et identification des problèmes
- [x] Création de AGENTS.md pour documentation des agents
- [x] Création de Cargo.toml avec dépendances exactes
- [x] Création de env.example pour configuration
- [x] Création de .gitignore pour Rust/Docker
- [x] Création du dossier examples/ avec basic_usage.md
- [x] Mise à jour complète de README.md avec documentation détaillée
- [x] Nettoyage des warnings (imports inutilisés supprimés)
- [x] Tests de compilation réussis (`cargo check`, `cargo build --release`)
- [x] Chargement automatique des fichiers AGENTS.md et README.md en début de discussion, si les fichiers sont présents

### 🔄 Tâches en Cours
- [x] Streaming des réponses (implémenté avec gestion buffer et correction du parsing)

### 📋 Amélioration possibles
- Touche "Echap" pour stopper un traitement en cours
- Améliorer l'interface utilisateur (historique de commandes, coloration)
- Ajouter des tests unitaires et d'intégration
- Documenter l'API interne et les décisions techniques
- Tests unitaires pour le streaming des tool_calls

## 🚀 Commandes Utiles

### Pour tester la compilation
```bash
# Vérifier la syntaxe
cargo check

# Compiler en mode debug
cargo build

# Compiler en mode release (optimisé)
cargo build --release

# Exécuter (avec variable d'environnement)
DEEPSEEK_API_KEY=your_key_here cargo run
```

### Pour développer
```bash
# Formatter le code
cargo fmt

# Vérifier les warnings
cargo clippy

# Exécuter les tests (à créer)
cargo test
```

## 💡 Notes pour les Agents Futurs

### À garder en tête
- L'utilisateur ne veut pas de Docker pour le moment
- Priorité : fonctionnalité de base avant optimisations
- Garder le code simple et lisible
- Documenter les décisions techniques

### Pièges à éviter
- Ne pas sur-engineerer trop tôt
- Vérifier que `rustyline` fonctionne dans tous les environnements
- Tester les appels API réels avec une clé valide

## 🔄 Dernière Mise à Jour

**Date** : 2026-03-17  
**Agent** : Assistant IA  
**Contexte** : Correction de l'erreur de parsing "missing field `name`" pour les réponses streaming. Implémentation de structures de désérialisation optionnelles pour les tool_calls streaming (`ToolCallDelta`, `FunctionCallDelta`). Ajout d'un système d'accumulation pour construire les tool_calls complets. Le streaming est maintenant désactivé par défaut (configurable via `DEEPSEEK_AGENT_STREAM`).

**Prochaines étapes** : 
1. Améliorer la gestion du streaming des tool_calls (tests supplémentaires)
2. Ajouter des tests unitaires pour les fonctions critiques
3. Améliorer l'interface utilisateur (historique de commandes, coloration syntaxique)
4. Ajouter une touche "Echap" pour stopper un traitement en cours

**État du projet** : ✅ **Fonctionnel (correction streaming)** - Prêt pour une utilisation en production avec streaming optionnel, gestion d'erreurs robuste, et configuration complète.

---

*Ce fichier doit être mis à jour à chaque session de travail significative.*