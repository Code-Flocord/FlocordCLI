# FlocordCLI

Installeur en ligne de commande pour [Flocord](https://github.com/Code-Flocord/Flocord), un client Discord modifié.

## Téléchargement

Récupère la dernière version depuis les [releases](https://github.com/Code-Flocord/FlocordCLI/releases/latest).

Aucune installation requise — lance simplement `FlocordCLI.exe`.

## Fonctionnalités

- Détection automatique de Discord, Discord PTB et Discord Canary
- Installation, réparation et désinstallation de Flocord
- Fermeture automatique de Discord avant installation
- Backup automatique de `app.asar` avant toute modification
- Support OpenAsar
- Journal d'installation sur le Bureau (`Flocord Logs/installer.log`)

## Utilisation

```
[1] Détecter Discord
[2] Installer Flocord
[3] Réparer Flocord
[4] Désinstaller Flocord
[5] Installer OpenAsar
[6] Désinstaller OpenAsar
[7] Quitter
```

## Build depuis les sources

### Prérequis

- [Rust](https://rustup.rs/)
- `dist/desktop.asar` depuis [FlocordCore](https://github.com/Code-Flocord/Flocord)

### Étapes

```bash
git clone https://github.com/Code-Flocord/FlocordCLI
cd FlocordCLI

# Copie le mod compilé depuis FlocordCore
copy ..\Flocord\dist\desktop.asar assets\desktop.asar

# Compile
cargo build --release
```

L'exécutable se trouve dans `target/release/FlocordCLI.exe`.

## Windows uniquement

FlocordCLI cible exclusivement Windows (Discord PTB / Stable / Canary).
