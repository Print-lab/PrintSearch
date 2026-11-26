# PrintSearch

[English](README.md) · **Français**

Une seule barre de recherche pour les modèles d'impression 3D de **Thingiverse**, **Printables**, **MakerWorld**, **Thangs**, **Cults** et **MyMiniFactory**.
Une petite application Rust avec une interface web locale et un mode ligne de commande : recherchez sur les six sites en même temps, filtrez et triez les résultats, likez un modèle ou ajoutez-le à une collection, et retrouvez vos recherches récentes. Ce code a été écrit par Claude afin de garantir une transparence totale ; je n'aurais jamais pu le faire tout seul : cela m'aurait pris trop de temps et je n'ai pas les connaissances nécessaires.

![Capture d'écran de PrintSearch 1](printsearch.png)

> **Non affilié à Thingiverse, Printables, MakerWorld, Thangs, Cults ni MyMiniFactory.** Les noms et logos appartiennent à leurs propriétaires et ne servent qu'à identifier chaque source. Voir [Limites et mentions légales](#limites-et-mentions-légales).

<!-- Ajoutez une capture d'écran ici : ![PrintSearch](docs/screenshot.png) -->

## Fonctionnalités

- Recherche en parallèle sur **Thingiverse, Printables, MakerWorld, Thangs, Cults et MyMiniFactory** ; les résultats sont entrelacés par site et s'affichent dès que chaque site répond (un site lent ne retarde jamais les autres).
- Filtres : **sites, prix (gratuit / payant), tri (pertinence / likes / téléchargements), likes minimum, téléchargements minimum, résultats par site**. Changer un filtre relance la recherche automatiquement.
- **Charger plus de résultats** (pagination) et petit cache pour que les recherches répétées soient rapides.
- **Like** et **ajout à une collection** depuis les cartes de résultats (voir [fonctionnalités par site](#fonctionnalités-par-site)).
- **Historique de recherche** sous la barre de recherche (désactivable) et **choix de la langue** (anglais par défaut ; français et allemand disponibles).
- Local par défaut : le serveur n'écoute que sur `127.0.0.1` et refuse les requêtes qui ne viennent pas de `localhost:8787` (protection contre les pages web malveillantes) ; vos jetons ne quittent pas votre ordinateur. Voir [Accéder depuis une autre machine](#accéder-depuis-une-autre-machine-facultatif) pour une configuration réseau facultative.
- **Tourne en arrière-plan, sans fenêtre de terminal :** une **icône dans la barre d'état** (Windows) / zone de notification (Linux) ouvre l'interface, la console ou ferme l'application.
- Une **console** intégrée (⚙ Paramètres → Paramètres avancés) qui accepte les mêmes commandes que le terminal, et une **commande `logs`** pour afficher ou exporter le journal de l'application (rapports de bug).
- **Vérification des mises à jour** auprès de ce dépôt GitHub (Paramètres, ligne de commande, et une discrète notification au démarrage que vous pouvez désactiver). Rien n'est téléchargé ni installé automatiquement.
- Utilisable aussi en **ligne de commande**.

## Prérequis

| Quoi | Pourquoi |
|---|---|
| [Rust](https://rustup.rs) (stable) | pour compiler depuis les sources |
| Un **navigateur basé sur Chromium** : Chrome, **Edge**, Brave, Vivaldi ou Chromium | MakerWorld, Thangs, Cults et MyMiniFactory n'ont pas d'API publique exploitable : ils sont lus avec un navigateur sans interface. Thingiverse s'en sert en repli quand vous n'avez pas de jeton. **Windows est livré avec Edge**, il n'y a donc en général rien à installer. Firefox et Safari ne sont pas pris en charge. Vous choisissez lequel dans ⚙ Paramètres. |
| `rustfmt` (inclus dans une installation `rustup` normale) | nécessaire pour compiler la dépendance `headless_chrome` ; si Rust a été installé avec le profil *minimal*, lancez `rustup component add rustfmt` |
| Windows uniquement : *Visual Studio Build Tools* (« Desktop development with C++ ») | requis par Rust sous Windows |

Pas besoin d'OpenSSL (le projet utilise `rustls`).

## Installation

### Depuis les sources (Linux, macOS, Windows)

```bash
git clone https://github.com/Print-lab/PrintSearch.git
cd PrintSearch
cargo build --release
```

Le binaire est `target/release/printsearch` (`target\release\printsearch.exe` sous Windows). L'interface web et les icônes y sont intégrées : un seul fichier.

### Paquet Debian / Ubuntu (`.deb`)

```bash
cargo install cargo-deb
cargo deb
sudo apt install ./target/debian/printsearch_*_amd64.deb
```

Le paquet *recommande* un navigateur basé sur Chromium (Chromium, Chrome, Edge ou Brave), donc `apt` proposera d'en installer un. Après l'installation, « PrintSearch » apparaît dans le menu des applications et la commande `printsearch` est disponible dans le terminal. Avant de distribuer le paquet, renseignez `maintainer` dans `Cargo.toml`.

### Serveur Linux sans écran (systemd)

Sur une machine sans écran, lancez PrintSearch comme service. L'icône de la barre d'état n'y sert à rien : démarrez avec `serve --no-tray`.

```bash
sudo apt install git curl build-essential chromium      # Ubuntu : chromium-browser
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/Print-lab/PrintSearch.git && cd PrintSearch/printsearch
cargo build --release
```

Une unité systemd utilisateur (`~/.config/systemd/user/printsearch.service`) :

```ini
[Unit]
Description=PrintSearch

[Service]
ExecStart=%h/PrintSearch/printsearch/target/release/printsearch serve --no-tray
Restart=on-failure
# only for network access, see below:
#Environment=PRINTSEARCH_BIND=192.168.1.20
#Environment=PRINTSEARCH_ALLOWED_HOSTS=192.168.1.20

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now printsearch
sudo loginctl enable-linger "$USER"      # continue de tourner après la déconnexion
```

- Le journal est `~/.local/share/printsearch/logs/printsearch.log` ; `printsearch logs` l'affiche.
- Les commandes `printsearch status` et `printsearch quit` doivent s'exécuter avec le même environnement (par exemple le même `PRINTSEARCH_BIND`) que le service.
- Une seule instance tourne par utilisateur.
- Pour atteindre l'interface depuis une autre machine, voir [Accéder depuis une autre machine](#accéder-depuis-une-autre-machine-facultatif).

### Exécutable Windows

L'`.exe` embarque l'icône de l'application. 
Compilez sous Windows avec `cargo build --release`, puis copiez `target\release\printsearch.exe` où vous voulez. L'application tourne sans fenêtre de console : utilisez son icône de la barre d'état ou `printsearch quit` pour l'arrêter. SmartScreen ou le pare-feu peuvent afficher une alerte au premier lancement (le serveur n'écoute qu'en local, vous pouvez autoriser).

### Compiler l'`.exe` Windows depuis Linux

```bash
sudo apt install mingw-w64
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

L'icône est intégrée avec `windres` (fourni par `mingw-w64`) ; s'il manque, la compilation réussit quand même, simplement sans icône (un avertissement s'affiche).

## Mise à jour

**⚙ Paramètres → Mises à jour → Rechercher des mises à jour** (ou `printsearch update`) compare votre version à la dernière version publiée sur GitHub. Par défaut, l'application vérifie aussi une fois au démarrage et affiche une petite notification si une version plus récente existe ; décochez *Vérifier automatiquement au démarrage* pour le désactiver. **Rien n'est téléchargé ni installé à votre place** ; pour mettre à jour :

- **Depuis les sources :** `git pull`, puis `cargo build --release`.
- **Debian / Ubuntu :** téléchargez le nouveau `.deb` depuis la [page des Releases](https://github.com/Print-lab/PrintSearch/releases) et lancez `sudo apt install ./printsearch_*_amd64.deb`.
- **Windows :** remplacez `printsearch.exe` par le nouveau.

La vérification ne contacte que GitHub (`api.github.com`, ou `github.com` si la limite de requêtes de l'API est atteinte) et n'envoie rien d'autre qu'une requête standard.

## Utilisation

### Interface web

![Capture d'écran de PrintSearch 2](printsearch-2.png)

Lancez **PrintSearch** depuis le menu des applications (ou `printsearch` sans argument). Il démarre en arrière-plan **sans aucune fenêtre de terminal**, affiche son **icône dans la barre d'état** et ouvre votre navigateur sur **http://localhost:8787**. Le relancer ouvre simplement l'instance déjà en cours. Lancé depuis un terminal, il **se détache tout seul** et vous rend l'invite, donc aucune fenêtre de terminal n'a besoin de rester ouverte (Linux).

```bash
printsearch                 # application en arrière-plan + icône + ouvre le navigateur
printsearch serve           # idem, sans ouvrir le navigateur (ajoutez --open pour l'ouvrir)
printsearch serve --foreground # reste attaché à ce terminal, Ctrl+C pour arrêter
printsearch serve --no-tray # sans icône : tourne dans ce terminal, Ctrl+C pour arrêter
```

Tapez une recherche et validez avec Entrée. Les filtres sont sous la barre de recherche ; le bouton ⚙ (en haut à droite) ouvre la page **Paramètres**.

### Icône de la barre d'état

**Clic gauche** : ouvre l'interface ; **clic droit** : menu *Ouvrir PrintSearch*, *Quitter*. On peut aussi quitter depuis ⚙ Paramètres → Application, avec la commande `quit`, ou avec `printsearch quit` dans un terminal.

- **Windows :** l'icône peut être rangée dans la zone de débordement (la flèche `^` près de l'horloge) ; glissez-la sur la barre des tâches pour la garder visible.
- **Linux :** l'icône utilise le protocole StatusNotifierItem, affiché par KDE, XFCE, Cinnamon, MATE… Sous **GNOME**, installez l'extension *AppIndicator and KStatusNotifierItem Support*. Sans zone de notification, PrintSearch fonctionne quand même en arrière-plan : utilisez l'interface web, ou `printsearch quit` pour l'arrêter.
- **macOS :** pas d'icône dans cette version ; PrintSearch tourne dans le terminal (comme avec `serve --no-tray`).
- La langue du menu suit celle de l'interface, appliquée au prochain démarrage.
- Pour compiler sans icône (programme de terminal uniquement) : `cargo build --release --no-default-features`.

### Console et commandes

**⚙ Paramètres → Paramètres avancés → Ouvrir la console** ouvre dans l'application une page de type terminal. Elle exécute **les mêmes commandes que le terminal** : tapez `help` pour les lister. `↑`/`↓` parcourent l'historique, `Tab` complète, `clear` efface l'écran, `lang en|de|fr` change la langue. Une commande contenant un jeton n'est jamais gardée dans l'historique et est masquée à l'écran et dans le journal.

### Ligne de commande

```bash
printsearch search "benchy"
printsearch search "support casque" --sites thingiverse,printables -l 5
printsearch search "vase" --price free --sort likes --min-likes 100 --page 2

printsearch login thingiverse                      # OAuth (voir plus bas)
printsearch login printables --token <JETON>
printsearch like thingiverse 763622
printsearch collect thingiverse <ID_COLLECTION> 763622

printsearch update                                 # vérifie sur GitHub s'il existe une version plus récente
printsearch status                                 # version, navigateur, connexions, emplacement du journal
printsearch logs                                   # 50 dernières lignes du journal (-n 200, --level warn)
printsearch logs export                            # journal complet + en-tête de diagnostic -> dossier Téléchargements
printsearch quit                                   # ferme le PrintSearch en cours
printsearch --version
```

Tout cela fonctionne aussi dans la console de l'application. Sous **Windows**, le programme n'a pas de fenêtre de console propre : lancé depuis un terminal, il s'y rattache et affiche son résultat, mais l'invite peut revenir avant la sortie (utilisez `printsearch.exe ... | more`, ou PowerShell, pour attendre).

`-l/--limit` est le nombre de résultats *par site* et *par page* ; `--page` commence à 1.

### Accéder depuis une autre machine (facultatif)

Par défaut, PrintSearch n'écoute que sur `127.0.0.1`. Pour l'utiliser depuis un autre ordinateur (par exemple un serveur sans écran), définissez deux variables d'environnement **avant de le démarrer** (dans l'unité systemd avec des lignes `Environment=`, dans un shell avec `export`) :

```bash
export PRINTSEARCH_BIND=192.168.1.20            # adresse d'écoute ; 0.0.0.0 ou :: = toutes les interfaces
export PRINTSEARCH_ALLOWED_HOSTS=192.168.1.20   # noms/adresses que les navigateurs peuvent utiliser, séparés par des virgules
```

- Écrivez les noms d'hôte en minuscules, sans port et sans `http://`. Seul `http` simple est pris en charge.
- Avec une écoute générale (`0.0.0.0`, `::`), l'adresse affichée est `localhost`.
- La connexion est en **HTTP simple, sans authentification** : toute personne qui atteint le port peut chercher et utiliser vos comptes connectés. Ne l'utilisez que sur un réseau de confiance, ou derrière un pare-feu / VPN.
- PrintSearch refuse ces actions aux clients qui ne sont pas sur l'ordinateur de PrintSearch lui-même : un chemin de navigateur personnalisé, `logs export` et `logs clear`, `quit`, et la connexion OAuth de Thingiverse. Pour récupérer le journal depuis une autre machine, utilisez le bouton **Télécharger les journaux** de l'interface.
- Une requête est locale si elle vient de la boucle locale ou d'une adresse de la même machine. Cela inclut les autres utilisateurs de cet ordinateur, un tunnel SSH et un proxy inverse sur la même machine : ils sont aussi fiables que l'ordinateur lui-même.
- La redirection OAuth de Thingiverse est fixée à `http://localhost:8787/callback`. Avec une écoute sur une adresse du réseau local, rien n'écoute sur `localhost:8787` : redirigez le port (`ssh -L 8787:<IP-du-serveur>:8787 <serveur>`) et ouvrez `http://localhost:8787` via le tunnel. Ou utilisez plutôt un App Token Thingiverse, qui fonctionne de partout.

### Choisir le navigateur utilisé pour les recherches

⚙ **Paramètres → Navigateur utilisé pour les recherches** : *Automatique* (le premier trouvé, dans l'ordre Chrome, Edge, Brave, Vivaldi, Chromium), un navigateur détecté, ou un **chemin personnalisé** vers n'importe quel exécutable basé sur Chromium. En ligne de commande :

```bash
printsearch browser                 # liste les navigateurs détectés et le choix actuel
printsearch browser edge            # chrome | edge | brave | vivaldi | chromium | auto | /chemin/du/navigateur
```

Le navigateur tourne en arrière-plan (sans fenêtre), images désactivées pour aller plus vite ; votre navigation personnelle n'est pas touchée.

## Connecter vos comptes

**La recherche fonctionne sans compte** sur Printables et MakerWorld. La connexion n'est utile que pour les actions sur votre compte (like, collections) et, sur Thingiverse, pour une recherche plus rapide et plus complète.

Tout se passe dans **⚙ Paramètres → Connexion aux sites** : chaque site a un badge d'état et un champ pour coller un jeton. Cliquez sur **Enregistrer le jeton** ; **Se déconnecter** le supprime.

### Thingiverse

Sans jeton, la recherche lit le site avec un navigateur en arrière-plan (plus lent ; sans auteur, likes ni téléchargements). Avec un jeton, elle utilise l'API officielle : rapide, avec auteur, likes et téléchargements. Un jeton est **obligatoire pour Like et les collections**.

**Option A, App Token (la plus simple).**
1. Connectez-vous sur <https://www.thingiverse.com/developers> et créez une app.
2. Copiez son **App Token**.
3. Collez-le dans Paramètres → Thingiverse → *Enregistrer le jeton*.

**Option B, connexion OAuth.**
1. Créez une app sur <https://www.thingiverse.com/developers> et définissez l'URI de redirection `http://localhost:8787/callback`.
2. Définissez vos identifiants **avant** de lancer l'application :

   ```bash
   # Linux / macOS
   export THINGIVERSE_CLIENT_ID="..."
   export THINGIVERSE_CLIENT_SECRET="..."
   printsearch serve
   ```
   ```powershell
   # Windows PowerShell
   $env:THINGIVERSE_CLIENT_ID="..."; $env:THINGIVERSE_CLIENT_SECRET="..."
   .\printsearch.exe serve
   ```
3. Dans les Paramètres, un bouton **Se connecter avec Thingiverse** apparaît. Après autorisation, vous revenez sur la page Paramètres.

Le bouton OAuth n'apparaît pas quand l'application est lancée depuis un menu ou par double-clic (pas de variables d'environnement) : utilisez alors l'option A. L'équivalent en ligne de commande est `printsearch login thingiverse`.

Vous pouvez aussi définir la variable d'environnement `THINGIVERSE_APP_TOKEN` au lieu d'enregistrer un jeton.

### Printables

La recherche fonctionne sans connexion. Pour utiliser **Like**, collez un jeton Bearer de votre session de navigateur :

1. Connectez-vous sur <https://www.printables.com>.
2. Ouvrez les outils de développement (<kbd>F12</kbd>) → onglet **Réseau**, puis rechargez la page.
3. Cliquez sur une requête vers `api.printables.com` ; dans **En-têtes de requête**, copiez la valeur après `Authorization: Bearer `.
4. Collez-la dans Paramètres → Printables → *Enregistrer le jeton*.

Ces jetons **expirent** : si Like se met à échouer, recommencez. Traitez un jeton comme un mot de passe.

### MakerWorld

La recherche fonctionne sans connexion (navigateur basé sur Chromium requis). Like et collections ne sont **pas encore implémentés** pour MakerWorld ; le champ existe dans les Paramètres pour plus tard et n'a pour l'instant aucun effet.

### Où sont stockés les jetons

Dans un fichier `config.json` du dossier de configuration de l'application (typiquement `~/.config/printsearch/` sous Linux, `%APPDATA%\printsearch\printsearch\config\` sous Windows, `~/Library/Application Support/` sous macOS), avec des droits réservés au propriétaire sous Unix. La désinstallation ne le supprime pas. La **langue et l'historique de recherche** sont enregistrés à part, dans votre navigateur (`localStorage` de `localhost:8787`).

## Journaux et rapports de bug

PrintSearch tient un journal de ce qu'il fait (recherches, erreurs, démarrage du navigateur, mises à jour). Il **ne contient jamais vos jetons**, mais il contient aussi vos termes de recherche, des chemins de fichiers et les sites connectés, et reste sur votre ordinateur. Emplacement : `~/.local/share/printsearch/logs/` sous Linux, `%LOCALAPPDATA%\printsearch\printsearch\data\logs\` sous Windows (`printsearch logs path` l'affiche). Il est limité à environ 2 Mo (une rotation).

```bash
printsearch logs                  # 50 dernières lignes
printsearch logs -n 200 --level warn
printsearch logs export           # fichier dans le dossier Téléchargements, avec un en-tête de diagnostic
printsearch logs export C:\temp\log.txt
printsearch logs clear
```

Dans l'application, ⚙ Paramètres → Paramètres avancés → **Télécharger les journaux** (ou le bouton de la console) enregistre le même fichier. Joignez-le à votre rapport de bug (relisez-le d'abord : il contient vos recherches).

## Fonctionnalités par site

| | Thingiverse | Printables | MakerWorld | Thangs | Cults | MyMiniFactory |
|---|---|---|---|---|---|---|
| Recherche sans connexion | ✅ via un navigateur (plus lent, données limitées) | ✅ | ✅ via un navigateur (données limitées) | ✅ via un navigateur (données limitées) | ✅ via un navigateur (données limitées) | ✅ via un navigateur (données limitées) |
| Recherche avec jeton | ✅ API officielle (rapide, données complètes) | n/a | n/a | n/a | n/a | n/a |
| Auteur / likes / téléchargements | ✅ avec jeton | ✅ | ❌ | ❌ | ❌ | ❌ |
| Filtre de prix | ✅ toujours gratuit | ⚠️ au mieux | ❌ inconnu | ❌ inconnu | ❌ inconnu | ❌ inconnu |
| Like | ⚠️ avec jeton, expérimental | ⚠️ avec jeton, expérimental | ❌ | ❌ | ❌ | ❌ |
| Ajout à une collection | ⚠️ avec jeton, expérimental | ❌ | ❌ | ❌ | ❌ | ❌ |

**Thangs, Cults et MyMiniFactory** se recherchent sans compte : PrintSearch lit leur page de résultats avec le navigateur d'arrière-plan et affiche le titre, l'image et le lien de chaque modèle. Il n'y a rien à connecter pour eux (ils n'apparaissent pas dans la liste des connexions des Paramètres), et leurs cartes n'ont que le lien *Ouvrir*.

⚠️ *Expérimental* : les points d'accès de Printables et MakerWorld sont **non officiels**, et les routes like/collection de Thingiverse ont été écrites d'après la description de l'API publique sans être vérifiées avec un vrai compte. Elles peuvent demander des ajustements. Les pull requests sont les bienvenues.

Quand une valeur est inconnue (par exemple les likes sur MakerWorld), les filtres « likes min / téléchargements min » n'excluent pas le modèle, et un prix inconnu est traité comme gratuit par le filtre *Gratuit*.

## Dépannage

- **« Aucun navigateur basé sur Chromium trouvé »** : installez Chrome, Edge, Brave ou Chromium, ou choisissez-en un (ou saisissez son chemin) dans ⚙ Paramètres → *Navigateur utilisé pour les recherches*. `printsearch browser` liste ce qui a été détecté.
- **« Le navigateur ne démarre pas (délai de 35 s) »** : le navigateur choisi n'est probablement pas basé sur Chromium (ou le chemin est faux). Choisissez-en un autre dans les Paramètres.
- **MakerWorld ou Thingiverse ne renvoie rien / « aucun résultat lu »** : le site affiche probablement une vérification anti-bot (par exemple une page intitulée « Just a moment… ») que le navigateur automatisé ne passe pas toujours. Pour Thingiverse, connectez-vous avec un App Token pour passer par l'API. Le bandeau d'erreur affiche le titre de la page lue, ce qui aide au diagnostic.
- **Thingiverse est lent sans jeton** : c'est le repli par navigateur ; utilisez un App Token.
- **Printables renvoie une erreur GraphQL ou rien** : l'API non officielle a peut-être changé ; ouvrez une issue avec le message.
- **Like échoue** : votre jeton a peut-être expiré ou la route a changé (voir *Fonctionnalités par site*).
- **Rien de visible au lancement** : PrintSearch tourne en arrière-plan ; cherchez l'icône dans la barre d'état, ou ouvrez <http://localhost:8787>. Si le port est déjà pris par un autre programme, `printsearch logs` l'explique.
- **Pas d'icône sous GNOME** : installez l'extension *AppIndicator* (voir *Icône de la barre d'état*). En attendant, utilisez l'interface web et `printsearch quit`.
- **Port 8787 utilisé** par un autre programme : le port est fixe (la redirection OAuth de Thingiverse en dépend). Fermez ce programme. (Lancer PrintSearch deux fois ne pose pas de problème : le second lancement ouvre simplement le premier.)
- **« limite de requêtes GitHub atteinte »** (vérification des mises à jour) : GitHub limite les appels anonymes à son API par adresse IP. L'application se rabat alors sur la page publique des releases ; si les deux échouent, réessayez plus tard.
- **« invalid peer certificate » / erreurs TLS** derrière un proxy d'entreprise ou un antivirus qui inspecte le HTTPS : l'application utilise le magasin de certificats du système, installez-y le certificat racine de votre organisation.

## Limites et mentions légales

- MakerWorld, Thangs, Cults, MyMiniFactory et Printables sont consultés par des moyens **non officiels** (un navigateur sans interface qui lit leurs pages, et un point d'accès GraphQL non documenté pour Printables). Les adresses de recherche et la forme des liens de Thangs, Cults et MyMiniFactory n'ont pas pu être vérifiées avec une vraie session au moment de leur ajout. Ceux-ci peuvent changer ou être bloqués à tout moment, et leurs conditions d'utilisation peuvent restreindre l'accès automatisé. Utilisez PrintSearch pour un **usage personnel, avec un volume de requêtes raisonnable**, et consultez les conditions de chaque site. Vous êtes responsable de votre usage.
- Aucune affiliation ni approbation de la part de Thingiverse (MakerBot/UltiMaker), Printables (Prusa Research) ou MakerWorld (Bambu Lab). Les marques et logos appartiennent à leurs propriétaires.
- Les résultats, miniatures et modèles appartiennent à leurs auteurs ; respectez toujours la licence de chaque modèle.

## Structure du projet

```
src/main.rs            CLI (clap) et point d'entrée
src/app.rs             cycle de vie : instance unique, thread du serveur, icône, arrêt propre
src/tray.rs            icône de la barre d'état / zone de notification (Windows, Linux)
src/commands.rs        commandes partagées par le terminal et la console intégrée
src/log.rs             fichier de journal, rotation, export
src/server.rs          serveur web local (axum) + API JSON
src/search.rs          recherche multi-sites, fusion, filtres, tri
src/browser.rs         lecteur de pages sans interface (détection, instance partagée, cache)
src/providers/         un fichier par site (Thingiverse, Printables, MakerWorld) + web.rs (Thangs, Cults, MyMiniFactory)
src/config.rs          stockage des jetons
web/index.html         tout le front-end (intégré au binaire)
web/icons/             icônes des sites (à remplacer puis recompiler)
packaging/             lanceur .desktop et icône pour le .deb
```

Ajouter un site, c'est ajouter dans `src/providers/` un fichier qui implémente le trait `Provider`.

## Historique des versions

Voir [CHANGELOG.fr.md](CHANGELOG.fr.md).

## Contribuer

Les [issues](https://github.com/Print-lab/PrintSearch/issues) et pull requests sont bienvenues, en particulier pour vérifier ou corriger les points d'accès expérimentaux, ajouter des sites et des traductions (l'objet `I18N` dans `web/index.html`).

## Licence

PrintSearch est un logiciel libre, publié sous licence **GNU General Public License v3.0** (GPL-3.0). Voir le fichier [LICENSE](LICENSE).
