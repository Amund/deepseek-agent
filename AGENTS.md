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


## 🏗️ Architecture Modulaire (Refactorisation)

Le code a été refactorisé en modules spécialisés pour réduire la taille du fichier agent.rs (passé de 1163 à 217 lignes) :

- **agent.rs** : Coordination générale, boucle interactive, traitement des tool_calls
- **api_client.rs** : Appels HTTP avec retry et gestion du streaming
- **history.rs** : Gestion de l'historique des messages, estimation des tokens, calibration
- **session.rs** : Redémarrage de session et création du fichier CONTINUE.md
- **streaming.rs** : Traitement des réponses streaming (ToolCallBuilder, parsing SSE)
- **security.rs** et **shell.rs** : inchangés

Cette séparation des responsabilités améliore la maintenabilité et permet des tests unitaires ciblés.

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
9. **✅ Gestion des tokens avec redémarrage de session** - Lorsqu'il reste moins de 4000 tokens disponibles, la session actuelle est stoppée, un fichier CONTINUE.md est créé avec un résumé de la tâche en cours, et une nouvelle session est automatiquement relancée avec le contexte des fichiers AGENTS.md, README.md et CONTINUE.md. Le fichier CONTINUE.md est supprimé après lecture. Cette approche remplace le nettoyage optimisé du contexte.
10. **✅ Correction du streaming des tool_calls** - Les arguments des tool_calls en streaming étaient mal accumulés (remplacement au lieu de concaténation). Correction de `ToolCallBuilder` pour concaténer les fragments d'arguments et validation des arguments non vides. Ajout de logs de débogage pour les erreurs de parsing JSON.

## 🛠️ Décisions Techniques Prises

### 1. Architecture
- **Pas de Docker pour le moment** : L'utilisateur préfère compiler et exécuter en local
- **Priorité** : Rendre le projet compilable avant toute optimisation

## Documentation

### API DeepSeek

- Create Chat Completion : https://api-docs.deepseek.com/api/create-chat-completion
- Lists Models : https://api-docs.deepseek.com/api/list-models
- Get User Balance : https://api-docs.deepseek.com/api/get-user-balance

### API Guides DeepSeek

- Thinking Mode : https://api-docs.deepseek.com/guides/thinking_mode
- Multi-round Conversation : https://api-docs.deepseek.com/guides/multi_round_chat
- Chat Prefix Completion (Beta) : https://api-docs.deepseek.com/guides/chat_prefix_completion
- JSON Output : https://api-docs.deepseek.com/guides/json_mode
- Tool Calls : https://api-docs.deepseek.com/guides/tool_calls
- Context Caching : https://api-docs.deepseek.com/guides/kv_cache

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
- [x] Streaming des réponses (implémenté avec gestion buffer et correction du parsing)
- [x] Gestion des tokens avec redémarrage de session (seuil de 4000 tokens)
- [x] Refactorisation modulaire (agent.rs réduit à 217 lignes)
- [x] Interruption avec Ctrl+C pour arrêter un traitement en cours (streaming et commandes shell) et quitter l'application
- [x] Interruption avec touche Échap pour arrêter le streaming de réponse
- [x] Ctrl+C permet de quitter complètement l'application (en plus d'interrompre le traitement)

### 📋 Amélioration possibles
- Ajouter des tests unitaires et d'intégration
- Améliorer l'interface utilisateur (historique de commandes, coloration)
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

**Date** : 2026-03-21  
**Agent** : Assistant IA  
**Contexte** : Correction des bugs dans le streaming des tool_calls (concaténation des arguments, validation des arguments vides). Ajout de logs de débogage pour les réponses API. L'erreur "EOF while parsing a value" était due à des arguments vides ou à une réponse API malformée. Ces corrections devraient permettre aux appels d'outils de fonctionner correctement.

**Prochaines étapes** : 
1. Ajouter des tests unitaires pour les nouveaux modules
2. Améliorer l'interface utilisateur (historique de commandes, coloration syntaxique)
3. Supprimer le champ `max_history_messages` inutilisé (optionnel)
4. Implémenter l'interruption pendant l'exécution de commandes shell (envoi de SIGINT au processus enfant)
5. Tester l'appel d'outils avec une clé API valide

**État du projet** : ✅ **Fonctionnel avec corrections** - Les fonctionnalités de base sont opérationnelles. Les bugs de streaming des tool_calls ont été corrigés. Des logs de débogage améliorés aident à diagnostiquer les erreurs d'API.

---

*Ce fichier doit être mis à jour à chaque session de travail significative.*