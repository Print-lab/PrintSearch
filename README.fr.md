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
- Strictement local : le serveur n'écoute que sur `127.0.0.1` et refuse les requêtes qui ne viennent pas de `localhost:8787` (protection contre les pages web malveillantes) ; vos jetons ne quittent pas votre ordinateur.
- **Tourne en arrière-plan, sans fenêtre de terminal :** une **icône dans la barre d'état** (Windows) / zone de notification (Linux) ouvre l'interface, la console ou ferme l'application.
- Une **console** intégrée (⚙ Paramètres → Paramètres avancés) qui accepte les mêmes commandes que le terminal, et une **commande `logs`** pour afficher ou exporter le journal de l'application (rapports de bug).
- **Vérification des mises à jour** auprès de ce dépôt GitHub (Paramètres, ligne de commande, et une discrète notification au démarrage que vous pouvez désactiver). Rien n'est téléchargé ni installé automatiquement.
- Utilisable aussi en **ligne de commande**.

## Prérequis

| Quoi | Pourquoi |
|---|---|
| [Rust](https://rustup.rs) (stable) | pour compiler depuis les sources |
| Un **navigateur basé sur Chromium** : Chrome, **Edge**, Brave, Vivaldi ou Chromium | MakerWorld et Thangs n'ont pas d'API publique exploitable : ils sont lus avec un navigateur sans interface. Thingiverse s'en sert en repli quand vous n'avez pas de jeton. **Windows est livré avec Edge**, il n'y a donc en général rien à installer. Firefox et Safari ne sont pas pris en charge. Vous choisissez lequel dans ⚙ Paramètres. |
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

### Exécutable Windows

L'`.exe` embarque l'icône de l'application. 
Compilez sous Windows avec `cargo build --release`, puis copiez `target\release\printsearch.exe` où vous voulez. Une fenêtre de console reste ouverte pendant l'utilisation : la fermer arrête le serveur. SmartScreen ou le pare-feu peuvent afficher une alerte au premier lancement (le serveur n'écoute qu'en local, vous pouvez autoriser).

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

**⚙ Paramètres → Paramètres avancés → Ouvrir la console** ouvre dans l'application une page de type terminal. Elle exécute **les mêmes commandes que le terminal** : tapez `help` pour les lister. `↑`/`↓` parcourent l'historique, `Tab` complète, `clear` efface l'écran, `lang en|fr` change la langue. Une commande contenant un jeton n'est jamais gardée dans l'historique et est masquée à l'écran et dans le journal.

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

### Cults

Cults est protégé par un contrôle anti-robot (Cloudflare) qui bloque la navigation automatisée : PrintSearch utilise donc son **API officielle**, qui demande une clé gratuite. Dans votre compte Cults, créez une clé d'API (<https://cults3d.com/en/api/keys>), puis saisissez votre **nom d'utilisateur** Cults et la **clé** dans **⚙ Paramètres → Cults**. En ligne de commande : `printsearch login cults3d --token <nom_utilisateur>:<cle_api>`. Recherche seulement (pas de Like ni de collections). Tant qu'aucune clé n'est enregistrée, Cults est ignoré et un message au-dessus des résultats l'explique.

### MyMiniFactory

MyMiniFactory bloque la navigation automatisée et demande aux applications d'utiliser son **API officielle**, qui demande une clé d'API gratuite : créez une application dans votre compte MyMiniFactory (<https://www.myminifactory.com/pages/for-developers>) et collez sa clé dans **⚙ Paramètres → MyMiniFactory** (ou `printsearch login myminifactory --token <cle_api>`). Recherche seulement. **Cette intégration n'a pas pu être testée avec une vraie clé quand elle a été écrite** : en cas d'erreur, le message affiché est la réponse de l'API ; signalez-le dans une issue pour qu'il soit corrigé.

### Où sont stockés les jetons

Dans un fichier `config.json` du dossier de configuration de l'application (typiquement `~/.config/printsearch/` sous Linux, `%APPDATA%\printsearch\printsearch\config\` sous Windows, `~/Library/Application Support/` sous macOS), avec des droits réservés au propriétaire sous Unix. La désinstallation ne le supprime pas. La **langue et l'historique de recherche** sont enregistrés à part, dans votre navigateur (`localStorage` de `localhost:8787`).

## Utilisation sur un serveur sans écran (Linux)

PrintSearch peut tourner sur une machine Linux sans écran (serveur domestique, Raspberry Pi…) et être utilisé depuis les autres ordinateurs de votre réseau.

1. Installez le `.deb` (ou copiez le binaire) et un navigateur basé sur Chromium (`sudo apt install chromium`) : MakerWorld, Thangs et Thingiverse sans jeton en ont besoin.
2. Lancez-le sans icône : `printsearch serve --no-tray`.
3. Pour y accéder depuis d'autres ordinateurs, indiquez où il écoute et quels noms d'hôte les clients utilisent :

   ```bash
   PRINTSEARCH_BIND=0.0.0.0 PRINTSEARCH_ALLOWED_HOSTS=printsearch.lan,192.168.1.20 printsearch serve --no-tray
   ```

   Ouvrez ensuite `http://printsearch.lan:8787` (ou l'adresse IP) depuis un autre ordinateur. `PRINTSEARCH_BIND` est une adresse IP (par défaut `127.0.0.1` : cet ordinateur seulement ; le port est toujours 8787). `PRINTSEARCH_ALLOWED_HOSTS` liste, séparés par des virgules, les noms d'hôte et adresses saisis dans les navigateurs des clients ; toute requête avec un autre `Host` ou `Origin` est refusée.

**Ce que les autres ordinateurs peuvent faire.** Il n'y a pas d'authentification : quand le serveur écoute au-delà de son propre ordinateur, **les clients des autres ordinateurs ne peuvent que rechercher**. Tout ce qui agit sur vos comptes ou administre l'application reste réservé à l'ordinateur qui fait tourner PrintSearch : enregistrer des jetons, Like et collections, le choix du navigateur, la console, les journaux et *Quitter*. Faites cette partie sur le serveur lui-même (`printsearch login …`, `printsearch browser …`) ou par un tunnel SSH (`ssh -L 8787:localhost:8787 serveur`, puis ouvrez `http://localhost:8787` : par le tunnel vous comptez comme local et avez tout). Ne placez **pas** devant PrintSearch un reverse proxy tournant sur la même machine sans authentification : ses requêtes sembleraient locales. N'exposez-le que sur un réseau de confiance, et ne redirigez jamais le port 8787 vers Internet. La connexion OAuth de Thingiverse ne fonctionne que sur le serveur lui-même (son adresse de retour est `localhost`) : utilisez-y un App Token.

Un service systemd minimal (`/etc/systemd/system/printsearch.service`) :

```ini
[Unit]
Description=PrintSearch
After=network-online.target

[Service]
User=printsearch
ExecStart=/usr/bin/printsearch serve --no-tray
Environment=PRINTSEARCH_BIND=0.0.0.0
Environment=PRINTSEARCH_ALLOWED_HOSTS=printsearch.lan,192.168.1.20
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Créez d'abord l'utilisateur (`sudo useradd -m printsearch`), puis `sudo systemctl enable --now printsearch`. Le journal est dans `~/.local/share/printsearch/logs/` de cet utilisateur (`sudo -u printsearch printsearch logs`).

## Journaux et rapports de bug

PrintSearch tient un journal de ce qu'il fait (recherches, erreurs, démarrage du navigateur, mises à jour). Il **ne contient jamais vos jetons**, mais il contient vos termes de recherche et reste sur votre ordinateur. Emplacement : `~/.local/share/printsearch/logs/` sous Linux, `%LOCALAPPDATA%\printsearch\printsearch\data\logs\` sous Windows (`printsearch logs path` l'affiche). Il est limité à environ 2 Mo (une rotation).

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
| Recherche sans connexion | ✅ via un navigateur (plus lent, données limitées) | ✅ | ✅ via un navigateur (données limitées) | ✅ via un navigateur (données limitées) | ❌ clé d'API gratuite nécessaire | ❌ clé d'API gratuite nécessaire |
| Recherche avec jeton | ✅ API officielle (rapide, données complètes) | n/a | n/a | n/a | ✅ API officielle (nom d'utilisateur + clé d'API) | ⚠️ API officielle (clé d'API), non testée |
| Auteur / likes / téléchargements | ✅ avec jeton | ✅ | ❌ | ❌ | ✅ | ⚠️ au mieux |
| Filtre de prix | ✅ toujours gratuit | ⚠️ au mieux | ❌ inconnu | ❌ inconnu | ✅ | ❌ inconnu |
| Like | ⚠️ avec jeton, expérimental | ⚠️ avec jeton, expérimental | ❌ | ❌ | ❌ | ❌ |
| Ajout à une collection | ⚠️ avec jeton, expérimental | ❌ | ❌ | ❌ | ❌ | ❌ |

**Thangs** se recherche sans compte : PrintSearch lit sa page de résultats avec le navigateur d'arrière-plan et affiche le titre, l'image et le lien de chaque modèle ; ses cartes n'ont que le lien *Ouvrir*. **Cults** et **MyMiniFactory** bloquent la navigation automatisée et demandent aux applications d'utiliser leurs API officielles : ils ne sont donc recherchés **qu'une fois une clé d'API gratuite ajoutée** dans les Paramètres (voir *Connecter vos comptes*) ; d'ici là ils sont ignorés et un message l'explique. Like et les collections ne sont pas disponibles pour ces trois sites.

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
- **Cults / MyMiniFactory ne sont pas recherchés** : ils demandent une clé d'API gratuite (⚙ Paramètres). Un message au-dessus des résultats l'indique. Si Cults répond *refused the credentials*, vérifiez votre nom d'utilisateur et votre clé d'API (la clé n'est pas votre mot de passe).
- **Pas d'icône sous GNOME** : installez l'extension *AppIndicator* (voir *Icône de la barre d'état*). En attendant, utilisez l'interface web et `printsearch quit`.
- **Port 8787 utilisé** par un autre programme : le port est fixe (la redirection OAuth de Thingiverse en dépend). Fermez ce programme. (Lancer PrintSearch deux fois ne pose pas de problème : le second lancement ouvre simplement le premier.)
- **« limite de requêtes GitHub atteinte »** (vérification des mises à jour) : GitHub limite les appels anonymes à son API par adresse IP. L'application se rabat alors sur la page publique des releases ; si les deux échouent, réessayez plus tard.
- **« invalid peer certificate » / erreurs TLS** derrière un proxy d'entreprise ou un antivirus qui inspecte le HTTPS : l'application utilise le magasin de certificats du système, installez-y le certificat racine de votre organisation.
- Les **messages d'erreur** venant du serveur (bandeau rouge) sont en anglais ; l'interface, elle, est traduite en anglais, français et allemand.

## Limites et mentions légales

- MakerWorld, Thangs et Printables sont consultés par des moyens **non officiels** (un navigateur sans interface qui lit leurs pages, et un point d'accès GraphQL non documenté pour Printables). Cults et MyMiniFactory sont consultés par leurs **API officielles**, avec votre propre clé. L'adresse de recherche et la forme des liens de Thangs, ainsi que les champs de l'API MyMiniFactory, n'ont pas pu être vérifiés avec une vraie session au moment de leur ajout. Ceux-ci peuvent changer ou être bloqués à tout moment, et leurs conditions d'utilisation peuvent restreindre l'accès automatisé. Utilisez PrintSearch pour un **usage personnel, avec un volume de requêtes raisonnable**, et consultez les conditions de chaque site. Vous êtes responsable de votre usage.
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

**Remerciements :** la traduction allemande, l'idée des réglages `PRINTSEARCH_BIND` / `PRINTSEARCH_ALLOWED_HOSTS`, la documentation du serveur sans écran et les commentaires et messages en anglais viennent d'une contribution de [@Leviathan09](https://github.com/Leviathan09) ([#1](https://github.com/Print-lab/PrintSearch/issues/1), [#2](https://github.com/Print-lab/PrintSearch/pull/2)), adaptée à cette version.

## Contribuer

Les [issues](https://github.com/Print-lab/PrintSearch/issues) et pull requests sont bienvenues, en particulier pour vérifier ou corriger les points d'accès expérimentaux, ajouter des sites et des traductions (l'objet `I18N` dans `web/index.html`).

## Licence

PrintSearch est un logiciel libre, publié sous licence **GNU General Public License v3.0** (GPL-3.0). Voir le fichier [LICENSE](LICENSE).
