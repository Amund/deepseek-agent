# Exemples d'utilisation - deepseek-agent

Ce fichier présente des exemples concrets d'interaction avec l'agent.

## 🚀 Lancement de l'agent

### Avec cargo run
```bash
export DEEPSEEK_API_KEY=votre_clé_api
cargo run --release
```

### Avec le binaire compilé
```bash
export DEEPSEEK_API_KEY=votre_clé_api
./target/release/deepseek-agent
```

## 💬 Exemples d'interaction

### 1. Commandes système de base

**Utilisateur :**
```
>> Quelle est la date et l'heure actuelles ?
```

**Agent :**
```
Agent: Je vais vérifier la date et l'heure pour vous.
[Shell] Exécution : date
Agent: La date et l'heure actuelles sont : Mon Dec 30 14:30:00 UTC 2024
```

---

### 2. Exploration du système de fichiers

**Utilisateur :**
```
>> Liste tous les fichiers du répertoire courant avec leurs permissions
```

**Agent :**
```
Agent: Je vais lister les fichiers avec leurs permissions.
[Shell] Exécution : ls -la
Agent: Voici la liste des fichiers :
total 48
drwxr-xr-x  4 user user  4096 Dec 30 14:00 .
drwxr-xr-x 20 user user  4096 Dec 30 13:00 ..
-rw-r--r--  1 user user  1250 Dec 30 14:00 README.md
-rw-r--r--  1 user user  2300 Dec 30 14:00 src/main.rs
drwxr-xr-x  2 user user  4096 Dec 30 14:00 target
```

---

### 3. Vérification des ressources système

**Utilisateur :**
```
>> Combien d'espace disque reste-t-il ?
```

**Agent :**
```
Agent: Je vais vérifier l'espace disque disponible.
[Shell] Exécution : df -h
Agent: Voici l'utilisation du disque :
Filesystem      Size  Used Avail Use% Mounted on
/dev/sda1        50G   20G   28G  42% /
tmpfs           3.9G     0  3.9G   0% /dev/shm
```

---

### 4. Recherche dans les fichiers

**Utilisateur :**
```
>> Cherche tous les fichiers Rust dans le projet qui contiennent le mot "agent"
```

**Agent :**
```
Agent: Je vais rechercher les fichiers Rust contenant "agent".
[Shell] Exécution : find . -name "*.rs" -exec grep -l "agent" {} \;
Agent: Voici les fichiers trouvés :
./src/main.rs
```

---

### 5. Opérations sur les processus

**Utilisateur :**
```
>> Montre les 5 processus qui utilisent le plus de mémoire
```

**Agent :**
```
Agent: Je vais vérifier les processus utilisant le plus de mémoire.
[Shell] Exécution : ps aux --sort=-%mem | head -6
Agent: Voici les processus utilisant le plus de mémoire :
USER       PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND
user      1234  2.5 15.2 1023456 789012 pts/0  Sl   14:00   0:05 /usr/bin/chrome
user      5678  1.2  8.7  654321 456789 pts/0  Sl   14:10   0:02 /usr/bin/code
```

---

### 6. Combinaison de commandes

**Utilisateur :**
```
>> Donne-moi un résumé du système : date, utilisateur, et répertoire courant
```

**Agent :**
```
Agent: Je vais vous donner un résumé du système.
[Shell] Exécution : echo "Date: $(date)" && echo "Utilisateur: $(whoami)" && echo "Répertoire: $(pwd)"
Agent: Voici le résumé :
Date: Mon Dec 30 14:35:00 UTC 2024
Utilisateur: user
Répertoire: /home/user/projects/deepseek-agent
```

---

### 7. Traitement de texte

**Utilisateur :**
```
>> Compte le nombre de lignes dans tous les fichiers Markdown du projet
```

**Agent :**
```
Agent: Je vais compter les lignes dans les fichiers Markdown.
[Shell] Exécution : find . -name "*.md" -exec wc -l {} \;
Agent: Voici le nombre de lignes par fichier :
  45 ./README.md
  30 ./TODO.md
  25 ./AGENTS.md
 100 total
```

## 🛡️ Utilisation avec liste blanche

Si vous activez la liste blanche (dans `src/main.rs`), seules les commandes autorisées fonctionneront :

**Configuration :**
```rust
let whitelist = Some(vec![
    "ls".to_string(),
    "date".to_string(),
    "echo".to_string(),
]);
```

**Interaction avec commande non autorisée :**
```
>> Supprime tous les fichiers (rm -rf /)
Agent: Je vais exécuter cette commande.
[Shell] Exécution : rm -rf /
Commande 'rm' non autorisée
```

## 🔧 Scénarios avancés

### Automatisation de tâches
L'agent peut être utilisé pour automatiser des séquences de commandes :

**Utilisateur :**
```
>> Crée un nouveau dossier "backup", copie tous les fichiers .rs dedans, puis liste le contenu
```

**Agent :** (exécutera les commandes séquentielles)

### Surveillance système
```
>> Surveille l'utilisation CPU toutes les 2 secondes pendant 10 secondes
```

### Analyse de logs
```
>> Affiche les 10 dernières lignes du journal système
```

## 💡 Conseils pour une meilleure interaction

1. **Soyez précis** : "liste les fichiers avec taille" vs juste "liste les fichiers"
2. **Utilisez un langage naturel** : L'agent comprend les requêtes en langage naturel
3. **Vérifiez les permissions** : L'agent exécute les commandes avec les permissions de l'utilisateur
4. **Testez d'abord** : Pour les commandes dangereuses, testez avec `echo` d'abord

## ⚠️ Limitations connues

1. **Pas de pipes interactifs** : Les commandes comme `vim` ou `top` ne fonctionneront pas
2. **Pas de commandes nécessitant une entrée utilisateur** 
3. **Sortie limitée** : Les commandes produisant beaucoup de sortie peuvent être tronquées
4. **Pas de sandboxing** : Les commandes s'exécutent avec les permissions de l'utilisateur