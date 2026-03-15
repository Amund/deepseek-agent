# deepseek-agent

Un agent CLI minimal écrit en Rust, utilisant l'API native de DeepSeek.  
Conçu pour être **léger, sans persistance** (tout en mémoire) et **destiné à tourner dans un conteneur Docker**.  
L'agent ne dispose que d'un seul outil : l'exécution de commandes shell (`sh`), ce qui en fait un assistant puissant mais simple, parfait pour des expérimentations ou des tâches automatisées dans un environnement isolé.

## ✨ Fonctionnalités

- **Appel à l'API DeepSeek** (modèle `deepseek-chat`) pour générer des réponses et décider d'utiliser l'outil shell.
- **Exécution de commandes shell** via l'outil `sh` (bash). Les commandes sont exécutées localement, sans sandboxing additionnel (l'isolation est déléguée au conteneur Docker).
- **Historique de conversation en mémoire** : aucune donnée n'est écrite sur le disque.
- **Liste blanche optionnelle** des commandes autorisées, configurable directement dans le code (par sécurité, même dans Docker).
- **Gestion complète des `tool_calls`** : l'agent peut appeler l'outil, recevoir le résultat, et poursuivre la conversation.
- **Interface en ligne de commande interactive** simple (saisie utilisateur, affichage des réponses).
- **Interruption avec Ctrl+C ou Échap** : permet d'arrêter un traitement en cours (streaming ou exécution de commande) et de retourner à l'invite utilisateur.

## 📋 Prérequis

- [Rust](https://www.rust-lang.org/) (dernière version stable) si vous souhaitez compiler vous-même.
- Une clé d'API DeepSeek (obtenable sur [platform.deepseek.com](https://platform.deepseek.com/)).
- Docker (optionnel, pour exécuter l'agent dans un conteneur).

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
Agent DeepSeek minimal (Docker). Tapez 'quit' pour sortir.
>> 
```

**Interruption** : Pendant qu'une réponse est générée (streaming) ou qu'une commande shell s'exécute, vous pouvez appuyer sur **Ctrl+C** ou **Échap** pour interrompre le traitement et retourner à l'invite `>>`.

Vous pouvez alors :
- Poser des questions en texte libre
- Demander l'exécution de commandes shell (ex: "liste les fichiers du répertoire courant")
- Taper `quit` pour quitter

### Exemples d'interaction

```
>> Quelle est la date actuelle ?
Agent: Je vais vérifier la date pour vous.
[Shell] Exécution : date
Agent: La date est: Mon Dec 30 12:00:00 UTC 2024
```

```
>> Liste les fichiers avec leurs tailles
Agent: Je vais lister les fichiers avec leurs tailles.
[Shell] Exécution : ls -lh
Agent: Voici la liste des fichiers :
-rw-r--r-- 1 user user 1.2K Dec 30 11:30 README.md
-rw-r--r-- 1 user user 2.3K Dec 30 11:30 src/main.rs
...
```

## ⚙️ Configuration

### Configuration de la sécurité

#### Via variables d'environnement (recommandé)
```bash
# Liste blanche (commandes autorisées)
export DEEPSEEK_AGENT_WHITELIST="ls,cat,echo,grep,find"

# Liste noire (commandes interdites)
export DEEPSEEK_AGENT_BLACKLIST="rm,shutdown,reboot,dd,mkfs"
```

#### Via modification du code (avancé)
Si vous préférez configurer directement dans le code, modifiez le fichier `src/main.rs` dans la fonction `main()` :

```rust
// Configuration dans le code
let whitelist = Some(vec![
    "ls".to_string(),
    "cat".to_string(),
    "echo".to_string(),
    "grep".to_string(),
]);
let blacklist = Some(vec![
    "rm".to_string(),
    "shutdown".to_string(),
]);
```

**Priorité** : La liste noire est vérifiée avant la liste blanche. Si une commande est à la fois dans les deux listes, elle sera interdite.

### Variables d'environnement

| Variable | Description | Défaut |
|----------|-------------|---------|
| `DEEPSEEK_API_KEY` | Clé API DeepSeek (requise) | - |
| `DEEPSEEK_AGENT_MODEL` | Modèle à utiliser (deepseek-chat, deepseek-reasoner, etc.) | `deepseek-chat` |
| `DEEPSEEK_AGENT_SYSTEM_PROMPT` | Prompt système personnalisé | Voir le code source |
| `DEEPSEEK_AGENT_WHITELIST` | Liste blanche de commandes (CSV) | Toutes autorisées |
| `DEEPSEEK_AGENT_BLACKLIST` | Liste noire de commandes (CSV) | Aucune interdite |
| `DEEPSEEK_AGENT_MAX_HISTORY_MESSAGES` | Nombre max de messages dans l'historique | Illimité |
| `DEEPSEEK_AGENT_MAX_CONTEXT_TOKENS` | Nombre max de tokens dans le contexte | Dépend du modèle :<br>- deepseek-chat: ~112 000<br>- deepseek-reasoner: ~104 000<br>- autres: 28 000 |
| `DEEPSEEK_AGENT_DEBUG` | Activer les logs de debug | Désactivé |
| `DEEPSEEK_AGENT_MAX_RETRIES` | Nombre maximum de tentatives pour les appels API | `3` |
| `DEEPSEEK_AGENT_RETRY_DELAY_MS` | Délai initial entre les tentatives (ms) | `1000` |
| `DEEPSEEK_AGENT_MAX_RETRY_DELAY_MS` | Délai maximum entre les tentatives (ms) | `30000` |
| `DEEPSEEK_AGENT_SHELL_TIMEOUT_MS` | Timeout pour l'exécution des commandes shell (ms) | Aucun |
| `DEEPSEEK_AGENT_SKIP_CONTEXT_FILES` | Désactiver le chargement automatique des fichiers AGENTS.md et README.md | Désactivé (fichiers chargés par défaut) |

**Calibration automatique** : L'agent estime automatiquement le nombre de tokens utilisés et ajuste ses estimations grâce aux données renvoyées par l'API DeepSeek. Cela permet une gestion précise du contexte et une optimisation du cache KV de DeepSeek.

Exemple de configuration :
```bash
export DEEPSEEK_API_KEY=votre_clé
export DEEPSEEK_AGENT_MODEL=deepseek-chat
export DEEPSEEK_AGENT_SYSTEM_PROMPT="Tu es un assistant spécialisé en DevOps."
export DEEPSEEK_AGENT_WHITELIST="ls,cat,echo,grep,find"
export DEEPSEEK_AGENT_BLACKLIST="rm,shutdown,reboot"
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

```
deepseek-agent/
├── src/main.rs          # Code source principal
├── Cargo.toml          # Configuration Rust
├── README.md           # Documentation
├── AGENTS.md           # Documentation pour les agents IA
├── env.example         # Template de configuration
└── .gitignore         # Fichiers à ignorer
```

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

# Exécuter les tests (à venir)
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

## 📄 Licence

MIT
