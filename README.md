# FlocordCLI

Installeur de [Flocord](https://github.com/Code-Flocord/Flocord), un client Discord modifié.

## Téléchargement

Récupère la dernière version depuis les [releases](https://github.com/Code-Flocord/FlocordCLI/releases/latest).

Aucune installation requise — lance simplement `FlocordCLI.exe`.

## Fonctionnalités

- Détection automatique de Discord Stable, PTB, Canary et Development
- Installation, réparation et désinstallation de Flocord
- Fermeture automatique de Discord avant installation
- Backup automatique de `app.asar` avant toute modification
- Protection automatique : Flocord est réparé au démarrage de Windows quand Discord s'est mis à jour
- Mises à jour signées : Flocord et l'installeur ne sont téléchargés que si leur empreinte correspond à la version signée
- Support OpenAsar
- Journal d'installation sur le Bureau (`Flocord Logs/installer.log`)

## Utilisation

Sans argument, l'installeur s'ouvre en fenêtre. `--cli` affiche le menu console, et chaque action existe aussi en ligne de commande :

```
FlocordCLI.exe [action] [options]

  --install              Installe Flocord
  --repair               Répare (réinstalle) Flocord
  --uninstall            Désinstalle Flocord et restaure Discord
  --status               Affiche l'état de chaque Discord
  --enable-protection    Active la réparation automatique au démarrage de Windows
  --disable-protection   Désactive la réparation automatique

  --channel <stable|ptb|canary>   Cible un seul Discord (sinon : tous)
  --silent, -s                    Aucune question, Discord est fermé et relancé si besoin
```

## Build depuis les sources

Prérequis : [Rust](https://rustup.rs/) et le `desktop.asar` de la version de Flocord indiquée dans `flocord.version`.

```bash
git clone https://github.com/Code-Flocord/FlocordCLI
cd FlocordCLI

# Le Flocord embarqué dans l'installeur
gh release download v$(cat flocord.version) -R Code-Flocord/Flocord -p desktop.asar -D assets

cargo test
cargo build --release
```

L'exécutable se trouve dans `target/release/FlocordCLI.exe`.

Les releases officielles sont construites par GitHub Actions à partir du tag, puis publiées une fois `version.json` signé.

## Windows uniquement

FlocordCLI cible exclusivement Windows.
