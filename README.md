# deepseek-agent

Un agent CLI minimal écrit en Rust, utilisant l'API native de DeepSeek.  
Conçu pour être **léger, modulaire et extensible**. L'agent dispose d'un seul outil : l'exécution de commandes shell (`sh`), ce qui en fait un assistant puissant pour l'automatisation de tâches système dans un environnement contrôlé.

## ✨ Fonctionnalités

- **Appels API DeepSeek** : Support complet des modèles `deepseek-chat` et `deepseek-reasoner` avec streaming des réponses
- **Exécution de commandes shell** via l'outil `sh` (bash) avec timeout configurable
- **Gestion intelligente du contexte** :
  - Estimation et calibration automatique des tokens
  - Optimisation du cache KV DeepSeek
  - Redémarrage automatique de session lorsque le contexte approche de la limite
- **Chargement automatique du contexte** : Les fichiers `AGENTS.md`, `README.md` et `CONTINUE.md` sont automatiquement chargés au démarrage
- **Gestion des interruptions** :
  - **Ctrl+C** : Interrompt un traitement en cours et quitte l'application
  - **Échap** : Interrompt le streaming d'une réponse

- **Gestion robuste des erreurs** :
  - Retry automatique avec backoff exponentiel pour les appels API
  - Logs de débogage détaillés
  - Timeout configurable pour l'exécution des commandes shell
- **Architecture modulaire** : Code organisé en 11 modules spécialisés pour une maintenabilité optimale
- **Support des tool_calls en streaming** : Accumulation correcte des fragments d'arguments JSON

## 🏗️ Architecture Modulaire

Le projet est organisé en modules Rust spécialisés :

- **agent.rs** : Coordination générale, boucle interactive, traitement des tool_calls
- **api.rs** : Définitions des structures de données pour l'API
- **api_client.rs** : Appels HTTP avec retry et gestion du streaming
- **config.rs** : Chargement de la configuration depuis les variables d'environnement
- **history.rs** : Gestion de l'historique des messages, estimation des tokens, calibration
- **interrupt.rs** : Gestion des interruptions (Ctrl+C, touche Échap)

- **session.rs** : Redémarrage de session et création du fichier CONTINUE.md
- **shell.rs** : Exécution des commandes shell avec timeout
- **streaming.rs** : Traitement des réponses streaming (ToolCallBuilder, parsing SSE)
- **token_management.rs** : Estimation des tokens et détermination des limites par modèle

## 📋 Prérequis

- [Rust](https://www.rust-lang.org/) (dernière version stable)
- Une clé d'API DeepSeek (obtenable sur [platform.deepseek.com](https://platform.deepseek.com/))
- Docker (optionnel, pour exécution en conteneur)

## 🚀 Installation

### Compilation depuis les sources

```bash
git clone https://github.com/votre-nom/deepseek-agent.git
cd deepseek-agent
cargo build --release
```

Le binaire compilé se trouvera dans `target/release/deepseek-agent`.

### Configuration de l'environnement

1. Copiez le fichier d'exemple de configuration :
   ```bash
   cp env.example .env
   ```

2. Éditez le fichier `.env` et ajoutez votre clé API DeepSeek :
   ```bash
   DEEPSEEK_API_KEY=votre_clé_api_ici
   ```

3. Optionnellement, configurez d'autres variables (voir section Configuration Avancée ci-dessous).

## 🎯 Utilisation

### Lancer l'agent

```bash
# Avec la variable d'environnement
export DEEPSEEK_API_KEY=votre_clé_api_ici
cargo run

# Ou en mode release
cargo run --release

# Ou directement le binaire compilé
./target/release/deepseek-agent
```

### Interface interactive

Une fois lancé, l'agent affiche :
```
Agent DeepSeek. Tapez 'quit' pour sortir.
>> 
```

**Interruption** : Pendant qu'une réponse est générée (streaming) ou qu'une commande shell s'exécute, vous pouvez appuyer sur **Ctrl+C** pour interrompre le traitement et quitter l'application, ou sur **Échap** pour interrompre seulement le streaming et retourner à l'invite.

Vous pouvez alors :
- Poser des questions en texte libre
- Demander l'exécution de commandes shell (ex: "liste les fichiers du répertoire courant")
- Taper `quit` pour quitter

### Exemples d'interaction

Voir le fichier [examples/basic_usage.md](examples/basic_usage.md) pour des exemples détaillés.

## ⚙️ Configuration Avancée

Toutes les options sont configurables via variables d'environnement :

| Variable | Description | Défaut |
|----------|-------------|---------|
| `DEEPSEEK_API_KEY` | Clé API DeepSeek (requise) | - |
| `DEEPSEEK_AGENT_MODEL` | Modèle à utiliser (deepseek-chat, deepseek-reasoner, etc.) | `deepseek-chat` |
| `DEEPSEEK_AGENT_SYSTEM_PROMPT` | Prompt système personnalisé | Voir le code source |
| `DEEPSEEK_AGENT_MAX_HISTORY_MESSAGES` | Nombre max de messages dans l'historique | Illimité |
| `DEEPSEEK_AGENT_MAX_CONTEXT_TOKENS` | Nombre max de tokens dans le contexte | Dépend du modèle :<br>- deepseek-chat: 128k<br>- deepseek-reasoner: 128k<br>- autres: 32k |
| `DEEPSEEK_AGENT_DEBUG` | Activer les logs de debug | Désactivé |
| `DEEPSEEK_AGENT_MAX_RETRIES` | Nombre maximum de tentatives pour les appels API | `3` |
| `DEEPSEEK_AGENT_RETRY_DELAY_MS` | Délai initial entre les tentatives (ms) | `1000` |
| `DEEPSEEK_AGENT_MAX_RETRY_DELAY_MS` | Délai maximum entre les tentatives (ms) | `30000` |
| `DEEPSEEK_AGENT_SHELL_TIMEOUT_MS` | Timeout pour l'exécution des commandes shell (ms) | Aucun |
| `DEEPSEEK_AGENT_STREAM` | Activer le streaming des réponses | Activé (true) |
| `DEEPSEEK_AGENT_SKIP_CONTEXT_FILES` | Désactiver le chargement automatique des fichiers AGENTS.md et README.md | Désactivé (fichiers chargés par défaut) |

**Calibration automatique** : L'agent estime automatiquement le nombre de tokens utilisés et ajuste ses estimations grâce aux données renvoyées par l'API DeepSeek. Cela permet une gestion précise du contexte et une optimisation du cache KV de DeepSeek.

Exemple de configuration complète :
```bash
export DEEPSEEK_API_KEY=votre_clé
export DEEPSEEK_AGENT_MODEL=deepseek-chat
export DEEPSEEK_AGENT_SYSTEM_PROMPT="Tu es un assistant spécialisé en DevOps."
export DEEPSEEK_AGENT_MAX_HISTORY_MESSAGES=20
export DEEPSEEK_AGENT_MAX_CONTEXT_TOKENS=28000  # Défaut dépend du modèle (voir documentation)
export DEEPSEEK_AGENT_DEBUG=1
export DEEPSEEK_AGENT_MAX_RETRIES=3              # Nombre maximum de tentatives pour les appels API
export DEEPSEEK_AGENT_RETRY_DELAY_MS=1000        # Délai initial entre les tentatives (ms)
export DEEPSEEK_AGENT_MAX_RETRY_DELAY_MS=30000   # Délai maximum entre les tentatives (ms)
export DEEPSEEK_AGENT_SHELL_TIMEOUT_MS=5000      # Timeout pour l'exécution des commandes shell (ms)
export DEEPSEEK_AGENT_SKIP_CONTEXT_FILES=1       # Désactiver le chargement automatique des fichiers de contexte
```

## 🔧 Développement

### Structure du projet

Voir la section [Architecture Modulaire](#🏗️-architecture-modulaire) pour une description détaillée des modules.

### Commandes de développement

```bash
# Vérifier la syntaxe
cargo check

# Compiler en mode debug
cargo build

# Compiler en mode release
cargo build --release

# Formatter le code
cargo fmt

# Vérifier les warnings
cargo clippy

# Exécuter les tests unitaires
cargo test
```

## 🐛 Dépannage

### Erreur "DEEPSEEK_API_KEY not found"
Assurez-vous d'avoir défini la variable d'environnement :
```bash
export DEEPSEEK_API_KEY=votre_clé
```

### Erreur de compilation
Vérifiez que vous avez Rust installé :
```bash
rustc --version
```

### L'agent ne répond pas
Vérifiez votre connexion internet et la validité de votre clé API.

### Streaming des tool_calls échoue avec "EOF while parsing a value"
Cela peut être dû à des arguments vides dans les tool_calls. Activez le mode debug (`DEEPSEEK_AGENT_DEBUG=1`) pour plus de détails.

## 📄 Licence

MIT
