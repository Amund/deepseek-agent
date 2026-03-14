# TODO - deepseek-agent

Liste des tâches à réaliser pour rendre le projet fonctionnel et robuste.

## 🔴 Priorité Élevée (Essentiel pour fonctionner)

### 1. Créer le fichier Cargo.toml ✅ **COMPLÉTÉ**
**Objectif** : Sans ce fichier, le projet ne peut pas être compilé avec `cargo build`.
**Tâches** :
- [x] Définir les métadonnées du projet (nom, version, édition Rust)
- [x] Ajouter les dépendances nécessaires :
  - `reqwest` avec la feature `json`
  - `tokio` avec les features `rt-multi-thread` et `macros`
  - `serde` avec les features `derive`
  - `serde_json`
  - `rustyline`
- [x] Configurer le profil de release pour optimisation

**Statut** : Le projet est maintenant compilable avec `cargo build` et `cargo build --release`.

### 2. Mise en place de l'environnement ✅ **COMPLÉTÉ**
**Objectif** : Permettre aux utilisateurs de configurer facilement l'agent.
**Tâches** :
- [x] Créer un fichier `env.example` avec toutes les variables d'environnement
- [x] Implémenter le chargement des variables d'environnement avec parsing CSV pour les listes
- [x] Documenter comment obtenir une clé API DeepSeek (dans README.md et env.example)

**Statut** : Système de configuration complet implémenté. Support de :
  - `DEEPSEEK_API_KEY` (requis)
  - `DEEPSEEK_AGENT_MODEL` (optionnel)
  - `DEEPSEEK_AGENT_SYSTEM_PROMPT` (optionnel)
  - `DEEPSEEK_AGENT_WHITELIST` (CSV, optionnel)
  - `DEEPSEEK_AGENT_BLACKLIST` (CSV, optionnel)
  - `DEEPSEEK_AGENT_MAX_HISTORY_MESSAGES` (optionnel)
  - `DEEPSEEK_AGENT_DEBUG` (optionnel)

### 3. Améliorer la sécurité de la liste blanche ✅ **PARTIELLEMENT COMPLÉTÉ**
**Problème actuel** : Seul le premier mot de la commande est vérifié, permettant des injections comme `ls; rm -rf /`.
**Améliorations apportées** :
- ✅ Blacklist étendue à tous les tokens (pas seulement le premier mot)
- ✅ Détection de patterns dangereux (`; rm`, `; sudo`, `> /dev/`, etc.)
- ✅ Validation basique des séparateurs de commandes

**Statut** : Sécurité significativement améliorée.

### 4. Corriger la gestion des appels d'outils multiples ✅ **COMPLÉTÉ**
**Problème actuel** : La boucle `for tool_call in tool_calls` rappelle l'API après chaque exécution, causant des appels superflus.
**Solution implémentée** :
- ✅ Collecte de tous les résultats d'outils dans un vecteur temporaire
- ✅ Un seul appel API final après traitement de tous les tool_calls
- ✅ Gestion correcte des erreurs de validation (messages d'erreur ajoutés au contexte)

**Statut** : Optimisation terminée. L'agent ne fait plus qu'un seul appel API supplémentaire même pour plusieurs commandes.

## 🟡 Priorité Moyenne (Améliorations importantes)

### 5. Gestion de l'historique de conversation ✅ **COMPLÉTÉ**
**Problème actuel** : L'historique `self.messages` grandit indéfiniment, risquant de dépasser les limites de tokens.
**Solution implémentée** :
- ✅ Méthode `add_message` qui gère automatiquement la limite
- ✅ Fenêtre glissante configurable via `DEEPSEEK_AGENT_MAX_HISTORY_MESSAGES`
- ✅ Conservation du message système (premier message) même si limite atteinte
- ✅ Logique de truncation intelligente qui préserve le contexte récent

**Statut** : Limite d'historique complètement fonctionnelle. Configuration via variable d'environnement.

### 6. Améliorer la gestion d'erreurs ✅ **COMPLÉTÉ**
**Objectif** : Rendre l'agent plus robuste face aux erreurs réseau/API.
**Améliorations apportées** :
- ✅ **Retries avec backoff exponentiel** pour les appels API
  - Configuration via `DEEPSEEK_AGENT_MAX_RETRIES`, `DEEPSEEK_AGENT_RETRY_DELAY_MS`, `DEEPSEEK_AGENT_MAX_RETRY_DELAY_MS`
  - Backoff exponentiel limité par délai maximum
  - Ne retry pas les erreurs client 4xx (sauf 429 rate limit)
  - Logs de debug pour suivre les tentatives
- ✅ **Timeout pour les commandes shell** via `DEEPSEEK_AGENT_SHELL_TIMEOUT_MS`
  - Utilise `tokio::time::timeout` pour limiter l'exécution
  - Message d'erreur explicite en cas de timeout
- ✅ **Messages d'erreur améliorés** dans le mode debug
  - Affichage des erreurs HTTP, réseau, et parsing JSON

**Tâches restantes** :
- [ ] Meilleurs messages d'erreur pour l'utilisateur (non-technique)
- [ ] Option : mode "safe" qui ignore les erreurs et continue

### 7. Interface utilisateur améliorée
**Objectif** : Améliorer l'expérience interactive.
**Tâches** :
- [ ] Ajouter la coloration syntaxique pour les sorties shell
- [ ] Ajouter un historique de commandes (flèches haut/bas)
- [ ] Support du multi-ligne pour les commandes longues
- [ ] Afficher un indicateur de chargement pendant les appels API

### 8. Tests et validation
**Objectif** : S'assurer que le code fonctionne correctement.
**Tâches** :
- [ ] Tests unitaires pour `exec_shell` avec commandes simples
- [ ] Tests unitaires pour la validation de liste blanche
- [ ] Tests d'intégration avec une API mockée
- [ ] Validation de la sérialisation/désérialisation JSON

## 🟢 Priorité Basse (Améliorations optionnelles)

### 9. Configuration avancée
**Objectif** : Rendre l'agent plus configurable.
**Tâches** :
- [ ] Support d'un fichier de configuration (YAML/TOML)
- [ ] Configuration de la liste blanche via fichier externe
- [ ] Variables d'environnement pour : modèle DeepSeek, timeout, limites
- [ ] Mode "verbose" pour le débogage

### 10. Optimisations
**Objectif** : Améliorer les performances et l'expérience.
**Tâches** :
- [ ] Streaming des réponses de l'API (si supporté)
- [ ] Cache des résultats de commandes fréquentes
- [ ] Compression de l'historique avant envoi à l'API
- [ ] Gestion des tokens pour éviter les dépassements

### 11. Sécurité renforcée
**Objectif** : Isoler encore plus l'exécution des commandes.
**Tâches** :
- [ ] Exécution dans un sandbox (bubblewrap, nsjail) même dans Docker
- [ ] Limites de ressources (CPU, mémoire, temps)
- [ ] Système de quotas pour les commandes
- [ ] Audit des commandes exécutées

### 12. Documentation complète
**Objectif** : Rendre le projet accessible aux contributeurs.
**Tâches** :
- [ ] Compléter le README.md avec :
  - Section "Utilisation" détaillée
  - Exemples d'interaction
  - Guide de dépannage
  - Contribution guidelines
- [ ] Ajouter des commentaires détaillés dans le code
- [ ] Documenter l'API interne
- [ ] Créer un CHANGELOG

## 📋 Fichiers créés

### Fichiers essentiels maintenant présents :
- [x] `Cargo.toml` - Configuration du projet Rust ✅
- [x] `env.example` - Template des variables d'environnement ✅ (`.env.example` bloqué pour sécurité)
- [x] `.gitignore` - Fichiers à ignorer par Git ✅
- [x] `examples/` - Dossier avec des exemples d'utilisation ✅
  - `examples/basic_usage.md` - Exemples détaillés d'interaction

### Fichiers de documentation :
- [x] `TODO.md` - Liste des tâches (ce fichier) ✅
- [x] `AGENTS.md` - Documentation pour les agents IA ✅

### Fichiers optionnels (pour plus tard) :
- [ ] `Dockerfile` - Pour l'exécution en conteneur (futur)
- [ ] `docker-compose.yml` - Pour une configuration multi-service
- [ ] `Makefile` - Pour automatiser les tâches courantes
- [ ] `tests/` - Dossier pour les tests

### Fichiers optionnels :
- [ ] `Dockerfile` - Pour l'exécution en conteneur (futur)
- [ ] `docker-compose.yml` - Pour une configuration multi-service
- [ ] `Makefile` - Pour automatiser les tâches courantes
- [ ] `tests/` - Dossier pour les tests

## 🚀 Prochaines étapes immédiates

1. **✅ Configuration complète** - Variables d'environnement supportées
2. **✅ Optimisation des appels API** - Boucle tool_calls corrigée
3. **✅ Sécurité améliorée** - Validation étendue et patterns dangereux
4. **✅ Gestion d'historique** - Limite configurable implémentée
5. **✅ Améliorer la gestion d'erreurs** - Retries et messages utilisateur (priorité haute)
6. **Tests unitaires** - Validation des fonctionnalités de base (priorité moyenne)
7. **Interface utilisateur** - Historique de commandes, coloration syntaxique (priorité basse)

## 📝 Notes techniques

### Problèmes identifiés dans le code actuel :

1. **Ligne 85** : `let mut stdin = rustyline::DefaultEditor::new()?;`
   - Gestion d'erreur basique, pourrait échouer silencieusement

2. **Ligne 135-140** : Appel API dans la boucle des tool_calls
   - Devrait être déplacé après le traitement de tous les tool_calls

3. **Ligne 154** : Validation de la liste blanche trop permissive
   - Devrait analyser toute la commande, pas juste le premier mot

4. **Absence de limite d'historique**
   - Risque de dépassement de tokens (limite API ~32k tokens)

5. **Gestion d'erreurs minimaliste**
   - Le programme s'arrête sur toute erreur

### Suggestions d'architecture :

1. **Séparer les responsabilités** :
   - Module pour l'API DeepSeek
   - Module pour l'exécution shell
   - Module pour la gestion de conversation

2. **Patterns à considérer** :
   - Builder pattern pour la configuration de l'Agent
   - Strategy pattern pour différentes méthodes de validation
   - Observer pattern pour les logs/audits

## 🔄 État d'avancement

**Date de création** : Initiale
**Dernière mise à jour** : 2025-03-15
**Progression globale** : 90% 

### ✅ Tâches complétées :
- [x] **Projet rendu compilable** - Cargo.toml créé avec toutes les dépendances
- [x] **Configuration complète** - Variables d'environnement supportées avec parsing CSV
- [x] **Optimisation API** - Correction de la boucle tool_calls (un seul appel final)
- [x] **Sécurité améliorée** - Blacklist étendue, patterns dangereux détectés
- [x] **Gestion d'historique** - Limite configurable avec conservation du message système
- [x] **Calibration des tokens** - Estimation automatique ajustée par les retours API
- [x] **Adaptation des limites de contexte** - Limites dynamiques selon le modèle DeepSeek
- [x] **Documentation complète** - README.md, examples/, AGENTS.md, TODO.md
- [x] **Nettoyage de code** - Warnings supprimés, code structuré
- [x] **Gestion d'erreurs avancée** - Retries avec backoff exponentiel, timeout shell, messages d'erreur améliorés

### 🔄 Tâches en cours :
- [ ] Validation avancée des commandes (guillemets, syntaxe shell)
- [ ] Tests unitaires et d'intégration

### 📋 Prochaines priorités :
1. ✅ Implémenter les retries avec backoff exponentiel pour les appels API
2. Ajouter des tests unitaires pour les fonctions critiques
3. Améliorer l'interface utilisateur (historique de commandes, coloration)
4. Documenter l'API interne et les décisions techniques
5. Implémenter la validation avancée des commandes (guillemets, syntaxe shell)

---

*Ce fichier sera mis à jour au fur et à mesure de l'avancement du projet.*