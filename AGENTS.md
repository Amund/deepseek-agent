# deepseek-agent

Documentation pour les agents IA reprenant le projet. Ce fichier contient le contexte, l'état actuel, les décisions techniques et les prochaines étapes.

## 🎯 Contexte du Projet

**Nom** : deepseek-agent  
**Langage** : Rust  
**Objectif** : Un agent CLI minimal utilisant l'API DeepSeek avec un seul outil - l'exécution de commandes shell (`sh`).  
**Philosophie** : Léger, sans persistance (tout en mémoire), destiné à tourner dans un conteneur Docker, donc sans sandboxing supplémentaire pour les commandes exécutées.

## 🏗️ Architecture Modulaire

Le code a été refactorisé en modules spécialisés pour améliorer la maintenabilité et permettre des tests unitaires ciblés :

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
│   └── token_management.rs # Estimation tokens
├── Cargo.toml             # Configuration Rust
├── README.md              # Documentation utilisateur
├── AGENTS.md              # Ce fichier (documentation agents)
├── env.example            # Template de configuration
├── .gitignore             # Fichiers ignorés
├── Makefile               # Commandes Docker
├── docker-compose.yml     # Configuration Docker
├── tests/                 # Tests d'intégration (vide pour l'instant)
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

### 🔄 Dernière Mise à Jour

**Date** : 2026-03-22  
**Agent** : Assistant IA  
**Contexte** : Suppression de la validation de sécurité des commandes shell (listes blanche/noire, patterns dangereux). Cette validation était inutilement lourde et le programme est destiné à être utilisé dans un conteneur Docker, où la sécurité est assurée par le conteneur. Toutes les références à la sécurité ont été retirées du code et de la documentation.

**État du projet** : ✅ **Fonctionnel et simplifié** - Les fonctionnalités de base sont opérationnelles sans validation superflue. Le code est plus simple et maintenable. Tous les tests passent avec succès.

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

1. **Tests d'intégration** : Ajouter des tests d'intégration dans le dossier `tests/`
2. **Interface utilisateur améliorée** : Historique de commandes, coloration syntaxique
3. **Optimisations de performance** : Réduction de la consommation mémoire, parallélisation
4. **Fonctionnalités avancées** : Support de plusieurs outils, mode batch
5. **Documentation API** : Documentation détaillée des modules internes

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