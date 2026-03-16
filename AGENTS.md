# deepseek-agent

Documentation pour les agents IA reprenant le projet. Ce fichier contient le contexte, l'état actuel, les décisions techniques et les prochaines étapes.

## 🎯 Contexte du Projet

**Nom** : deepseek-agent  
**Langage** : Rust  
**Objectif** : Un agent CLI minimal utilisant l'API DeepSeek avec un seul outil - l'exécution de commandes shell (`sh`).  
**Philosophie** : Léger, sans persistance (tout en mémoire), destiné à tourner dans un conteneur Docker, donc sans sandboxing supplémentaire pour les commandes exécutées.

## 🏗️ Architecture Modulaire

Le code a été refactorisé en modules spécialisés pour améliorer la maintenabilité et permettre des tests unitaires ciblés. Le projet a été structuré en une librairie (`src/lib.rs`) et un binaire (`src/main.rs`) pour permettre des tests d'intégration et une meilleure organisation :

### Modules principaux

- **agent.rs** : Coordination générale, boucle interactive, traitement des tool_calls
- **api.rs** : Définitions des structures de données pour l'API (messages, requêtes, réponses)
- **api_client.rs** : Appels HTTP avec retry et gestion du streaming
- **config.rs** : Chargement de la configuration depuis les variables d'environnement
- **history.rs** : Gestion de l'historique des messages, estimation des tokens, calibration
- **interrupt.rs** : Gestion des interruptions (Ctrl+C, touche Échap)
- **session.rs** : Redémarrage de session et création du fichier CONTINUE.md
- **shell.rs** : Exécution des commandes shell avec timeout
- **streaming.rs** : Traitement des réponses streaming (ToolCallBuilder, parsing SSE)
- **token_management.rs** : Estimation des tokens et détermination des limites par modèle
- **ui.rs** : Interface utilisateur améliorée avec couleurs

### Structure des fichiers

```
deepseek-agent/
├── src/
│   ├── main.rs            # Point d'entrée, initialisation des modules
│   ├── agent.rs           # Logique principale de l'agent
│   ├── api.rs             # Structures API
│   ├── api_client.rs      # Client HTTP avec retries
│   ├── config.rs          # Configuration
│   ├── history.rs         # Gestion historique
│   ├── interrupt.rs       # Gestion interruptions
│   ├── session.rs         # Redémarrage session
│   ├── shell.rs           # Exécution shell
│   ├── streaming.rs       # Traitement streaming
│   ├── token_management.rs # Estimation tokens
│   └── ui.rs              # Interface utilisateur améliorée
├── Cargo.toml             # Configuration Rust
├── README.md              # Documentation utilisateur
├── AGENTS.md              # Ce fichier (documentation agents)
├── env.example            # Template de configuration
├── .gitignore             # Fichiers ignorés
├── Makefile               # Commandes Docker
├── docker-compose.yml     # Configuration Docker
├── tests/                 # Tests d'intégration (integration.rs)
└── examples/              # Exemples d'utilisation
    ├── basic_usage.md     # Exemples détaillés
    └── continue_example.md # Exemple de fichier CONTINUE.md
```

## ✨ Fonctionnalités Principales

### ✅ Fonctionnalités implémentées

1. **Appels API DeepSeek** : Support complet des modèles deepseek-chat et deepseek-reasoner
2. **Exécution de commandes shell** via l'outil `sh` (bash)
3. **Streaming des réponses** : Affichage en temps réel des réponses de l'API
4. **Gestion d'historique intelligente** :
   - Estimation et calibration des tokens
   - Optimisation du cache KV DeepSeek
   - Limites configurables (messages et tokens)
5. **Redémarrage automatique de session** : Création d'un fichier CONTINUE.md lorsque moins de 4000 tokens restent
6. **Chargement automatique du contexte** : AGENTS.md, README.md et CONTINUE.md chargés automatiquement au démarrage
7. **Gestion des interruptions** :
   - Ctrl+C pour arrêter un traitement et quitter l'application
   - Touche Échap pour interrompre le streaming d'une réponse

8. **Gestion robuste des erreurs** :
   - Retry automatique avec backoff exponentiel
   - Timeout configurable pour les commandes shell
   - Logs de débogage détaillés
9. **Support des tool_calls en streaming** : Accumulation correcte des fragments d'arguments JSON
10. **Interface utilisateur améliorée** : Coloration syntaxique, gestion des couleurs via variables d'environnement

### 🔧 Configuration avancée

- **Variables d'environnement** : Toutes les options configurables via variables d'environnement
- **Personnalisation du prompt système** : Support des prompts personnalisés
- **Limites de contexte par modèle** : Détermination automatique des limites de tokens
- **Cache KV DeepSeek** : Optimisation via calibration des tokens

## 📋 État d'Avancement

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
- [x] Correction des bugs dans le streaming des tool_calls (concaténation des arguments, validation des arguments vides)
- [x] Ajout de logs de débogage pour les réponses API
- [x] Tests unitaires pour les nouveaux modules
- [x] Suppression de la validation de sécurité (listes blanche/noire, patterns dangereux)
- [x] Tests d'intégration : Ajouter des tests d'intégration dans le dossier `tests/`
- [x] Interface utilisateur améliorée : Coloration syntaxique (historique persistant retiré pour respecter le contrat de légèreté)

### 🔄 Dernière Mise à Jour

**Date** : 2026-03-23  
**Agent** : Assistant IA  
**Contexte** : Suppression de l'historique persistant des commandes pour respecter le contrat de départ (programme léger et sans persistance). Conservation de la coloration syntaxique via le module `ui.rs` avec gestion des couleurs (variables DEEPSEEK_AGENT_NO_COLOR et DEEPSEEK_AGENT_COLOR). Amélioration de l'affichage des messages (agent, shell, erreurs) avec des couleurs.

**Suppression de `max_history_messages`** : Cette variable d'environnement était un reliquat d'une ancienne fonctionnalité. La gestion du contexte repose désormais entièrement sur le calcul des tokens (`max_context_tokens`). La variable a été retirée du code, de la configuration et de la documentation.

**État du projet** : ✅ **Fonctionnel et bien testé** - Les fonctionnalités de base sont opérationnelles avec des tests unitaires et d'intégration. Le code est structuré en librairie et binaire pour une meilleure maintenabilité. L'interface utilisateur offre une expérience améliorée avec couleurs, sans persistance pour rester léger.

### 🛠️ Décisions Techniques Prises

#### 1. Architecture Modulaire
- **Séparation des responsabilités** : Chaque module gère une fonctionnalité spécifique
- **Testabilité** : Les modules peuvent être testés unitairement
- **Maintenabilité** : Code plus lisible avec des responsabilités claires

#### 2. Gestion des Tokens
- **Estimation approximative** : Utilisation d'une heuristique basée sur le nombre de caractères
- **Calibration automatique** : Ajustement basé sur les comptes réels retournés par l'API
- **Redémarrage de session** : Lorsqu'il reste moins de 4000 tokens, création d'un fichier CONTINUE.md et redémarrage propre

#### 3. Streaming
- **Parsing SSE** : Traitement ligne par ligne des événements Server-Sent Events
- **Accumulation des fragments** : Concaténation correcte des arguments JSON pour les tool_calls
- **Interruption utilisateur** : Possibilité d'arrêter le streaming avec Échap

#### 4. Gestion des Erreurs
- **Retry avec backoff exponentiel** : Pour les erreurs réseau et de rate limiting
- **Logs de débogage** : Affichage conditionnel avec variable d'environnement
- **Interruption propre** : Gestion des signaux Ctrl+C et Échap

### 📈 Prochaines Étapes

- **Fonctionnalités avancées** : Support de plusieurs outils, mode batch
- **Documentation API** : Documentation détaillée des modules internes

### 🔍 Analyse des Optimisations Possibles (2026-03-23)

**Contexte** : L'agent est actuellement fonctionnel avec des performances adéquates pour un usage interactif. Les goulots d'étranglement principaux sont les appels réseau (API DeepSeek) et l'exécution des commandes shell, pas le traitement local.

#### Optimisations potentielles identifiées :

1. **Parallélisation des tool_calls** :
   - **Utilité** : Modérée. Permettrait d'exécuter plusieurs commandes shell simultanément quand l'assistant envoie plusieurs tool_calls dans un seul message.
   - **Complexité** : Moyenne. Nécessite de gérer la concurrence et de préserver l'ordre des résultats.
   - **Risque** : Les commandes pourraient avoir des dépendances (ex: `cd` puis `ls`). Cependant, l'assistant peut les envoyer séquentiellement s'il a besoin d'ordre.
   - **Recommandation** : Implémenter optionnellement avec une variable d'environnement `DEEPSEEK_AGENT_PARALLEL_TOOL_CALLS`.

2. **Réduction de la consommation mémoire** :
        - **Utilité** : Faible. L'historique est limité par `max_context_tokens`. Les messages ne contiennent pas de données volumineuses.
   - **Optimisations possibles** :
     - Utiliser `Box<str>` au lieu de `String` pour les champs de message (micro-optimisation).
     - Compresser les messages anciens (trop complexe).
   - **Recommandation** : Aucune action nécessaire.

3. **Optimisation du streaming** :
   - **Utilité** : Faible à modérée. Le streaming affiche caractère par caractère, ce qui peut être lent pour des réponses longues.
   - **Optimisations possibles** :
     - Bufferisation de l'affichage (afficher par lignes ou par blocs de N caractères).
     - Réutilisation des buffers pour réduire les allocations.
     - Travailler directement avec les bytes au lieu de convertir en UTF-8 à chaque chunk.
   - **Recommandation** : Implémenter un buffer d'affichage configurable (ex: 1024 caractères).

4. **Réduction des clones inutiles** :
   - **Utilité** : Faible. Les clones identifiés sont nécessaires pour la propriété des données. Quelques clones pourraient être évités avec des références ou des prises de possession.
   - **Recommandation** : Revue de code pour éliminer quelques clones évidents, mais impact minimal.

5. **Cache des estimations de tokens** :
   - **Utilité** : Très faible. L'estimation des tokens est une opération peu coûteuse (comptage de caractères).
   - **Recommandation** : Aucune action.

**Conclusion** :
- Les optimisations de performance ne sont pas critiques pour le moment.
- La priorité devrait rester sur les fonctionnalités avancées et la robustesse.
- Si des optimisations sont implémentées, elles devraient être optionnelles et mesurables.

**Décision** : Reporter les optimisations de performance au profit des fonctionnalités avancées (support de plusieurs outils, mode batch). Documenter cette analyse pour référence future.

### 💡 Notes pour les Agents Futurs

#### À garder en tête
- L'utilisateur ne veut pas de Docker pour le moment (mais les fichiers Docker sont présents)
- Priorité : fonctionnalité de base avant optimisations
- Garder le code simple et lisible
- Documenter les décisions techniques

#### Pièges à éviter
- Ne pas sur-engineerer trop tôt
- Vérifier que `rustyline` fonctionne dans tous les environnements
- Tester les appels API réels avec une clé valide
- Les tool_calls en streaming nécessitent une accumulation correcte des fragments d'arguments

#### Bonnes pratiques
- Toujours ajouter des tests unitaires pour les nouvelles fonctionnalités
- Mettre à jour AGENTS.md après chaque session de travail significative
- Vérifier la compilation avec `cargo check` avant de proposer des modifications
- Utiliser les variables d'environnement pour la configuration plutôt que du code dur

---

*Ce fichier doit être mis à jour à chaque session de travail significative.*